//! src/render/passes/hiz/clusters/ — per-cluster occlusion culling (#1968), the
//! culling half of UE5's Nanite on top of the two-phase Hi-Z (#1963).
//!
//! A mesh the import split into clusters (`asset::clusters`: at least
//! `MIN_TRIANGLES`, not skinned) uploads a cluster table beside its buffers
//! ([`GpuClusters`]). A camera that culls by Hi-Z then draws each opaque batch of
//! such a mesh **per instance, by cluster**, with no mesh shaders:
//!
//! - **Phase one** is unchanged: last frame's visible instances draw whole into
//!   the depth the pyramid is built from. No cluster history is kept, so phase one
//!   draws a little more than it must, never less.
//! - **Phase two**, after `cs_test` judged every instance against this frame's
//!   pyramid: `cs_compact` keeps a clustered batch's instances in place, and per
//!   clustered batch `hiz_clusters.wgsl` tests each (instance, cluster) by the
//!   frustum, its normal cone and the same pyramid (`hiz/hidden.wgsl`'s
//!   `hidden`), then compacts the kept clusters' indices, in order, into the
//!   camera's output index list, with one indirect draw per instance. A cluster
//!   that came out from behind a wall is tested against this frame's pyramid, so
//!   it is drawn this frame, as an instance is.
//!
//! **Never culled.** A cluster crossing the near plane; the clusters of an
//! instance the per-instance test does not judge (a moving material, a ringed
//! object: `CameraSolids::tested`); a cone test on a batch that does not keep
//! front faces only (`RenderFace::Both`, `Back`, a mirrored instance), or under a
//! world matrix that isn't a rotation and uniform scale. A cluster never spans two
//! draw runs (material slots), since the import splits each run on its own.
//!
//! **What it costs.** A clustered batch is one draw per instance rather than one
//! instanced draw, and its output list has room for every instance's whole run;
//! past [`plan::MAX_OUT_INDICES`] a batch is drawn by the instance test alone.
//! The quality preset's `gpu_cluster_culling` turns it off.

mod gpu;
mod layout;
pub(crate) mod plan;
mod run;

use std::ops::Range;
use std::sync::Arc;

use crate::render::gpu::pipelines::{lazy_compute, LazyCompute};

pub(crate) use gpu::GpuClusters;
pub(crate) use run::ClusterCull;

/// The final draws that stand for a camera's clustered batches: per opaque batch,
/// the jobs whose indirect args (at `ARGS` each in `args`) draw it from
/// `indices`, or `None` for a batch drawn whole.
#[derive(Clone)]
pub(crate) struct ClusterDraws {
    pub indices: wgpu::Buffer,
    pub args: wgpu::Buffer,
    pub draws: Arc<Vec<Option<Range<u32>>>>,
}

/// The cluster passes' layouts and pipelines, shared by every view.
pub(crate) struct ClusterPipelines {
    pub(crate) cull_layout: wgpu::BindGroupLayout,
    pub(crate) mesh_layout: wgpu::BindGroupLayout,
    pub(crate) test: LazyCompute,
    pub(crate) scan: LazyCompute,
    pub(crate) copy: LazyCompute,
}

impl ClusterPipelines {
    pub(crate) fn new(device: &wgpu::Device, module: &wgpu::ShaderModule) -> Self {
        let cull_layout = layout::cull_layout(device);
        let mesh_layout = layout::mesh_layout(device);
        let groups: &[&wgpu::BindGroupLayout] = &[&cull_layout, &mesh_layout];
        let with = |entry| lazy_compute(device, module, (entry, entry), Some(groups));
        Self {
            test: with("cs_cluster_test"),
            scan: with("cs_cluster_scan"),
            copy: with("cs_cluster_copy"),
            cull_layout,
            mesh_layout,
        }
    }
}

impl crate::render::Renderer {
    /// Which of `draws`' opaque batches this camera draws by cluster, with
    /// `tables` (the instance test's packed bounds) saying which instances are
    /// judged: none when the preset's `gpu_cluster_culling` is off.
    pub(crate) fn plan_clusters(
        &self,
        draws: &crate::render::draw::batch::FrameDraws,
        tables: &super::kept::Tables,
    ) -> plan::Plan {
        if !self.cluster_culls() {
            return plan::Plan::default();
        }
        let tested = |i: usize| tables.bounds.get(i).is_some_and(|b| b[3] > 0.5);
        let clusters_of = |b: &crate::render::draw::batch::DrawBatch| {
            let mesh = self.gpu_mesh_of(b.key.mesh)?.clusters.as_ref()?;
            mesh.covering(b.key.indices.range())
        };
        let instances = (&draws.instances[..], &tested as &dyn Fn(usize) -> bool);
        plan::Plan::of(draws.opaque.iter(), instances, clusters_of)
    }
}
