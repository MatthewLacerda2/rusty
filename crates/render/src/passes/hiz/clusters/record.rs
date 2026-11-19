//! Recording one camera's cluster cull (#2406): its three stages, stage by
//! stage, every group's test, then every scan, then every copy, the same order
//! whether the frame is timed (a stage a pass) or not (all three in one pass).

use super::layout::RUN_STRIDE;
use super::plan::Plan;
use super::run::{copy_at, ClusterCull};
use crate::timing::GpuPart;
use crate::Renderer;

impl Renderer {
    /// Record `plan`'s dispatches of `steps` (the cluster cull's sub-parts, in
    /// order) into `pass` in [`dispatch_order`], each group's group 0 at its own
    /// dynamic offset; `ready_clusters` ran first. An untimed frame passes all
    /// three steps, a timed one a step a pass: the same dispatches, in the same
    /// order, the timer adding only pass boundaries.
    pub(crate) fn record_clusters(
        &self,
        pass: &mut wgpu::ComputePass,
        (cull, plan): (&ClusterCull, &Plan),
        steps: &[GpuPart],
    ) {
        let cull_group = cull.group.value().expect("bound by ready_clusters");
        let copy_group = cull.copy_group.value().expect("bound by ready_clusters");
        let pipes = &self.hiz.clusters;
        for (step, g) in dispatch_order(steps, plan.groups.len()) {
            let group = &plan.groups[g];
            let mesh = self
                .gpu_mesh_of(group.mesh)
                .and_then(|m| m.clusters.as_ref());
            let mesh = &mesh
                .expect("planned this frame from a resident clustered mesh")
                .group;
            let offset = [g as u32 * RUN_STRIDE as u32];
            let jobs = group.jobs.len() as u32;
            pass.set_bind_group(1, mesh, &[]);
            #[cfg(test)]
            cull.dispatched.borrow_mut().push((step, g));
            match step {
                GpuPart::OcclusionClustersTest => {
                    pass.set_bind_group(0, cull_group, &offset);
                    pass.set_pipeline(pipes.test.get());
                    pass.dispatch_workgroups((jobs * group.clusters).div_ceil(64), 1, 1);
                }
                GpuPart::OcclusionClustersScan => {
                    pass.set_bind_group(0, cull_group, &offset);
                    pass.set_pipeline(pipes.scan.get());
                    pass.dispatch_workgroups(jobs, 1, 1);
                }
                GpuPart::OcclusionClustersCopy => {
                    // Sized by what the scan kept: no workgroup a culled cluster.
                    pass.set_bind_group(0, copy_group, &offset);
                    pass.set_pipeline(pipes.copy.get());
                    let at = 4 * u64::from(copy_at(g));
                    pass.dispatch_workgroups_indirect(cull.tally.buffer(), at);
                }
                other => unreachable!("{other:?} is no cluster cull dispatch"),
            }
        }
    }
}

/// The (step, group) dispatches `steps` record over `groups` dispatch groups:
/// stage by stage, every group's first step, then every group's next. Each
/// stage reads only what the stage before wrote for the same group, so the
/// order computes what batch by batch would, without a pipeline change and a
/// hand-over of `state` between every dispatch.
pub(crate) fn dispatch_order(
    steps: &[GpuPart],
    groups: usize,
) -> impl Iterator<Item = (GpuPart, usize)> + '_ {
    steps
        .iter()
        .flat_map(move |s| (0..groups).map(move |g| (*s, g)))
}
