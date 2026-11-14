//! A mesh's clusters on the GPU (#1968): the table `hiz_clusters.wgsl` reads,
//! made with the mesh's buffers at upload, and the CPU's copy of where each
//! cluster's indices start, which a batch's run is matched against.

use std::ops::Range;

use wgpu::util::DeviceExt;

use rusty_asset::clusters::Clusters;

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
    let one = |c: &rusty_asset::clusters::Cluster| {
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

/// Whether an index buffer of `bytes` can be bound whole as storage under a
/// device's `max_storage_buffer_binding_size` of `limit` (128 MiB by default,
/// about 33.5M indices): past it, binding it is a validation panic.
pub(super) fn binds(bytes: u64, limit: u64) -> bool {
    bytes <= limit
}

impl crate::Renderer {
    /// Give the resident mesh `id` the table of `clusters` (its import's, still
    /// describing its geometry), or none. Clusters that don't end at its last
    /// index are not its own, and are dropped; so are those of a mesh whose index
    /// buffer is past the device's storage binding limit ([`binds`]), which the
    /// cluster passes could not bind, and which then draws whole as before.
    pub(crate) fn attach_clusters(&mut self, id: &crate::MeshId, clusters: Option<&Clusters>) {
        let layout = &self.hiz.clusters.mesh_layout;
        let Some(mesh) = self.gpu_meshes.get_mut(id) else {
            return;
        };
        let limit = self.device.limits().max_storage_buffer_binding_size;
        let fits = |c: &&Clusters| {
            let own = c.list.last().map(|l| l.indices.end) == Some(mesh.num_indices);
            own && binds(mesh.index_buffer.size(), limit)
        };
        mesh.clusters = clusters
            .filter(fits)
            .map(|c| GpuClusters::new(&self.device, layout, c, &mesh.index_buffer));
    }
}

#[cfg(test)]
mod tests {
    use super::{binds, cover};

    #[test]
    fn a_mesh_past_the_storage_binding_limit_gets_no_cluster_table() {
        let limit = 128 << 20;
        assert!(binds(limit, limit), "exactly the limit binds");
        assert!(!binds(limit + 4, limit), "34M indices do not");
    }

    #[test]
    fn a_run_is_covered_only_when_it_starts_and_ends_on_cluster_edges() {
        let runs = [0..30, 30..60, 60..96];
        assert_eq!(cover(&runs, 30..96), Some(1..3));
        assert_eq!(cover(&runs, 0..96), Some(0..3));
        assert_eq!(cover(&runs, 3..60), None, "starts inside a cluster");
        assert_eq!(cover(&runs, 30..63), None, "ends inside one");
    }
}
