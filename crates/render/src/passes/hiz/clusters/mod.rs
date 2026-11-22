mod eye;
mod gpu;
mod layout;
pub(crate) mod plan;
pub(crate) mod pool;
mod ranges;
mod record;
mod run;

use std::ops::Range;
use std::sync::Arc;

use crate::gpu::pipelines::{lazy_compute, LazyCompute};

pub(crate) use gpu::GpuClusters;
pub(crate) use run::ClusterCull;

#[derive(Clone)]
pub(crate) struct ClusterDraws {
    pub indices: wgpu::Buffer,
    pub args: wgpu::Buffer,
    pub draws: Arc<Vec<Option<Range<u32>>>>,
}

pub(crate) struct ClusterPipelines {
    pub(crate) test_layout: wgpu::BindGroupLayout,
    pub(crate) scan_layout: wgpu::BindGroupLayout,
    pub(crate) copy_layout: wgpu::BindGroupLayout,
    pub(crate) pool_layout: wgpu::BindGroupLayout,
    pub(crate) pool: pool::ClusterPool,
    pub(crate) test: LazyCompute,
    pub(crate) scan: LazyCompute,
    pub(crate) copy: LazyCompute,
}

impl ClusterPipelines {
    pub(crate) fn new(device: &wgpu::Device, module: &wgpu::ShaderModule) -> Self {
        let test_layout = layout::test_layout(device);
        let scan_layout = layout::scan_layout(device);
        let copy_layout = layout::copy_layout(device);
        let pool_layout = layout::pool_layout(device);
        let with = |entry, group0| {
            let groups: &[&wgpu::BindGroupLayout] = &[group0, &pool_layout];
            lazy_compute(device, module, (entry, entry), Some(groups))
        };
        Self {
            test: with("cs_cluster_test", &test_layout),
            scan: with("cs_cluster_scan", &scan_layout),
            copy: with("cs_cluster_copy", &copy_layout),
            pool: pool::ClusterPool::new(device, &pool_layout),
            test_layout,
            scan_layout,
            copy_layout,
            pool_layout,
        }
    }
}

impl crate::Renderer {
    pub(crate) fn plan_clusters(
        &self,
        draws: &crate::draw::batch::FrameDraws,
        tables: &super::tables::Tables,
    ) -> plan::Plan {
        if !self.cluster_culls() {
            return plan::Plan::default();
        }
        let tested = |i: usize| tables.bounds.get(i).is_some_and(|b| b[3] > 0.5);
        let instances = (&draws.instances[..], &tested as &dyn Fn(usize) -> bool);
        let clusters_of = |b: &crate::draw::batch::DrawBatch| self.batch_clusters(b);
        plan::Plan::of(draws.opaque.iter(), instances, clusters_of)
    }

    pub(crate) fn batch_clusters(
        &self,
        batch: &crate::draw::batch::DrawBatch,
    ) -> Option<Range<u32>> {
        let mesh = self.gpu_mesh_of(batch.key.mesh)?.clusters.as_ref()?;
        mesh.covering(batch.key.indices.range())
    }

    pub(crate) fn draws_clustered(&self, draws: &crate::draw::batch::FrameDraws) -> bool {
        self.cluster_culls()
            && draws
                .opaque
                .iter()
                .any(|b| self.batch_clusters(b).is_some())
    }
}
