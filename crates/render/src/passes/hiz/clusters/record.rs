use super::plan::Plan;
use super::run::ClusterCull;
use crate::timing::GpuPart;
use crate::Renderer;

const STEP: u32 = 0;
const POOL: u32 = 1;

const MAX_DIMENSION: u32 = 65535;

impl Renderer {
    pub(crate) fn record_clusters(
        &self,
        pass: &mut wgpu::ComputePass,
        (cull, plan): (&ClusterCull, &Plan),
        steps: &[GpuPart],
    ) {
        let pipes = &self.hiz.clusters;
        let bound = "bound by ready_clusters";
        pass.set_bind_group(POOL, &pipes.pool.group, &[]);
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
            pass.set_bind_group(STEP, group.expect(bound), &[]);
            pass.set_pipeline(pipeline.get());
            pass.dispatch_workgroups(x, y, 1);
        }
    }
}

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
