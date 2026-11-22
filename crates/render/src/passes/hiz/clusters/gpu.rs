use std::ops::Range;

use rusty_asset::clusters::Clusters;

use super::pool::Lease;

pub(crate) struct GpuClusters {
    pub(crate) runs: Vec<Range<u32>>,
    pub(crate) lease: Lease,
}

pub(super) fn pack(clusters: &Clusters, base: u32) -> Vec<[f32; 16]> {
    let one = |c: &rusty_asset::clusters::Cluster| {
        let (first, count) = (base + c.indices.start, c.indices.len() as u32);
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
    pub(crate) fn covering(&self, run: Range<u32>) -> Option<Range<u32>> {
        let local = cover(&self.runs, run)?;
        let first = self.lease.table.start;
        Some(first + local.start..first + local.end)
    }
}

pub(super) fn cover(runs: &[Range<u32>], run: Range<u32>) -> Option<Range<u32>> {
    let lo = runs.partition_point(|c| c.start < run.start);
    let hi = runs.partition_point(|c| c.end <= run.end);
    let whole = lo < hi && runs[lo].start == run.start && runs[hi - 1].end == run.end;
    whole.then_some(lo as u32..hi as u32)
}

impl crate::Renderer {
    pub(crate) fn attach_clusters(
        &mut self,
        id: &crate::MeshId,
        (clusters, indices): (Option<&Clusters>, &[u32]),
    ) {
        self.retained.residency.refused.remove(id);
        let Some(mesh) = self.gpu_meshes.get_mut(id) else {
            return;
        };
        mesh.clusters = None;
        let own = |c: &&Clusters| {
            let end = c.list.last().map(|l| l.indices.end);
            end == Some(mesh.num_indices) && indices.len() as u32 == mesh.num_indices
        };
        let Some(clusters) = clusters.filter(own) else {
            return;
        };
        let pipes = &mut self.hiz.clusters;
        let at = (&self.device, &self.queue);
        let lease = pipes
            .pool
            .lease(at, &pipes.pool_layout, (clusters, indices));
        if lease.is_none() {
            self.retained.residency.refused.insert(id.clone());
            log::warn!(
                "[Render] {}: its {} clusters do not fit the cluster pool under the device's \
                 storage binding limit; it is drawn whole",
                id.0,
                clusters.list.len()
            );
        }
        mesh.clusters = lease.map(|lease| GpuClusters {
            runs: clusters.list.iter().map(|c| c.indices.clone()).collect(),
            lease,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::cover;

    #[test]
    fn a_run_is_covered_only_when_it_starts_and_ends_on_cluster_edges() {
        let runs = [0..30, 30..60, 60..96];
        assert_eq!(cover(&runs, 30..96), Some(1..3));
        assert_eq!(cover(&runs, 0..96), Some(0..3));
        assert_eq!(cover(&runs, 3..60), None, "starts inside a cluster");
        assert_eq!(cover(&runs, 30..63), None, "ends inside one");
    }
}
