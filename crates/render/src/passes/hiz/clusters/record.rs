//! Recording one camera's cluster cull (#2406, #2417): its three stages, one
//! dispatch each for the whole frame, the test, then the scan, then the copy,
//! the same whether the frame is timed (a stage a pass) or not (all three in
//! one pass).

use super::plan::Plan;
use super::run::ClusterCull;
use crate::timing::GpuPart;
use crate::Renderer;

/// A dispatch dimension's limit, in workgroups (wgpu's default
/// `max_compute_workgroups_per_dimension`).
const MAX_DIMENSION: u32 = 65535;

impl Renderer {
    /// Record `plan`'s dispatches of `steps` (the cluster cull's sub-parts, in
    /// order) into `pass`, each stage's group 0 and the pool's group 1;
    /// `ready_clusters` ran first. An untimed frame passes all three steps, a
    /// timed one a step a pass: the same dispatches, in the same order, the
    /// timer adding only pass boundaries.
    pub(crate) fn record_clusters(
        &self,
        pass: &mut wgpu::ComputePass,
        (cull, plan): (&ClusterCull, &Plan),
        steps: &[GpuPart],
    ) {
        let pipes = &self.hiz.clusters;
        let bound = "bound by ready_clusters";
        pass.set_bind_group(1, &pipes.pool.group, &[]);
        for &step in steps {
            let (group, pipeline, count) = match step {
                GpuPart::OcclusionClustersTest => {
                    (cull.test_group.value(), &pipes.test, plan.chunks.len())
                }
                GpuPart::OcclusionClustersScan => {
                    (cull.scan_group.value(), &pipes.scan, plan.jobs.len())
                }
                GpuPart::OcclusionClustersCopy => {
                    (cull.copy_group.value(), &pipes.copy, plan.chunks.len())
                }
                other => unreachable!("{other:?} is no cluster cull dispatch"),
            };
            let (x, y) = workgroups(count as u32);
            #[cfg(test)]
            cull.dispatched.borrow_mut().push((step, (x, y)));
            pass.set_bind_group(0, group.expect(bound), &[]);
            pass.set_pipeline(pipeline.get());
            pass.dispatch_workgroups(x, y, 1);
        }
    }
}

/// `count` workgroups as a dispatch's (x, y): one row up to [`MAX_DIMENSION`],
/// past it full rows and a second dimension, the shader flattening them back
/// (`flat` in `hiz_clusters.wgsl`) and skipping the last row's overhang.
pub(crate) fn workgroups(count: u32) -> (u32, u32) {
    let rows = count.div_ceil(MAX_DIMENSION).max(1);
    (count.div_ceil(rows), rows)
}

#[cfg(test)]
mod tests {
    use super::{workgroups, MAX_DIMENSION};

    #[test]
    fn a_dispatch_past_one_dimension_takes_a_second_and_covers_every_workgroup() {
        assert_eq!(workgroups(0), (0, 1), "nothing to run");
        assert_eq!(workgroups(7), (7, 1));
        assert_eq!(workgroups(MAX_DIMENSION), (MAX_DIMENSION, 1));
        for count in [MAX_DIMENSION + 1, 3 * MAX_DIMENSION + 5, 1 << 24] {
            let (x, y) = workgroups(count);
            assert!(x <= MAX_DIMENSION && y <= MAX_DIMENSION, "{count}: {x}x{y}");
            assert!(x * y >= count && x * (y - 1) < count, "{count}: {x}x{y}");
        }
    }
}
