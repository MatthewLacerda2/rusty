//! A mesh's clusters on the GPU (#1968): the table `hiz_clusters.wgsl` reads,
//! made with the mesh's buffers at upload, and the CPU's copy of where each
//! cluster's indices start, which a batch's run is matched against.

use std::ops::Range;

use wgpu::util::DeviceExt;

use crate::asset::clusters::Clusters;

/// A resident mesh's clusters: their GPU table and the group 1 the cluster
/// passes bind it and the mesh's indices through.
pub(crate) struct GpuClusters {
    /// Each cluster's index range, in order.
    pub(crate) runs: Vec<Range<u32>>,
    /// The table `group` binds.
    _table: wgpu::Buffer,
    pub(crate) group: wgpu::BindGroup,
}

/// `clusters` as `hiz_clusters.wgsl`'s `Cluster` rows (16 words each).
pub(super) fn pack(clusters: &Clusters) -> Vec<[f32; 16]> {
    let one = |c: &crate::asset::clusters::Cluster| {
        let (first, count) = (c.indices.start, c.indices.len() as u32);
        let [a, b, d] = c.min;
        let [e, f, g] = c.max;
        let [h, i, j] = c.apex;
        let [k, l, m] = c.axis;
        let (first, count) = (f32::from_bits(first), f32::from_bits(count));
        [
            a, b, d, first, e, f, g, count, h, i, j, c.cutoff, k, l, m, 0.0,
        ]
    };
    clusters.list.iter().map(one).collect()
}

impl GpuClusters {
    /// The table of `clusters` over `indices` (the mesh's index buffer, which
    /// they reordered), bound by `layout`.
    pub(crate) fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        clusters: &Clusters,
        indices: &wgpu::Buffer,
    ) -> Self {
        let rows = pack(clusters);
        let table = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Clusters"),
            contents: bytemuck::cast_slice(&rows),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mesh Clusters"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: table.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: indices.as_entire_binding(),
                },
            ],
        });
        let runs = clusters.list.iter().map(|c| c.indices.clone()).collect();
        Self {
            runs,
            _table: table,
            group,
        }
    }

    /// The clusters making up exactly the index run `run`, by their place in the
    /// table: `None` when the run does not start and end on a cluster's edge.
    pub(crate) fn covering(&self, run: Range<u32>) -> Option<Range<u32>> {
        cover(&self.runs, run)
    }
}

/// [`GpuClusters::covering`] over the cluster `runs`.
pub(super) fn cover(runs: &[Range<u32>], run: Range<u32>) -> Option<Range<u32>> {
    let lo = runs.partition_point(|c| c.start < run.start);
    let hi = runs.partition_point(|c| c.end <= run.end);
    let whole = lo < hi && runs[lo].start == run.start && runs[hi - 1].end == run.end;
    whole.then_some(lo as u32..hi as u32)
}

impl crate::render::Renderer {
    /// Give the resident mesh `id` the table of `clusters` (its import's, still
    /// describing its geometry), or none. Clusters that don't end at its last
    /// index are not its own, and are dropped.
    pub(crate) fn attach_clusters(
        &mut self,
        id: &crate::render::MeshId,
        clusters: Option<&Clusters>,
    ) {
        let layout = &self.hiz.clusters.mesh_layout;
        let Some(mesh) = self.gpu_meshes.get_mut(id) else {
            return;
        };
        let fits = |c: &&Clusters| {
            c.list
                .last()
                .is_some_and(|l| l.indices.end == mesh.num_indices)
        };
        mesh.clusters = clusters
            .filter(fits)
            .map(|c| GpuClusters::new(&self.device, layout, c, &mesh.index_buffer));
    }
}
