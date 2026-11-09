//! Which of a camera's opaque batches are drawn by cluster this frame (#1968),
//! and the jobs `hiz_clusters.wgsl` runs for them: one per instance, each with
//! its room in the output index list. Pure CPU work over the frame's batches.

use std::ops::Range;

use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::pipelines::winding::Facing;
use crate::render::scene::MeshHandle;
use crate::render::InstanceData;

/// The most output indices one camera's clustered draws may hold (32 MB): a
/// batch that would pass it is drawn whole, by the per-instance test alone.
pub(crate) const MAX_OUT_INDICES: u64 = 1 << 23;
/// A dispatch dimension's limit, in workgroups: a batch with more instances or
/// clusters than this, or more (instance, cluster) pairs than 64 times it (the
/// test's threads), is drawn whole.
const MAX_DIMENSION: u32 = 65535;

/// `hiz_clusters.wgsl`'s `Job`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Job {
    pub instance: u32,
    pub object: u32,
    pub tested: u32,
    pub out_first: u32,
    pub cluster_first: u32,
    pub state_first: u32,
    pub cone: u32,
    pub _pad: u32,
}

/// One clustered batch: its place among the opaque batches, its jobs, and how
/// many clusters each has.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    pub batch: usize,
    pub mesh: MeshHandle,
    pub jobs: Range<u32>,
    pub clusters: u32,
}

/// A camera's clustered draws this frame.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Plan {
    pub jobs: Vec<Job>,
    pub runs: Vec<Run>,
    /// Output indices and state words the jobs need; state word 0 is the
    /// count of clusters kept, so the jobs' start at 1.
    pub out_indices: u32,
    pub states: u32,
    /// The clusters of instances the per-instance test judges: `Debug.Stats`'
    /// `cluster_tested`.
    pub tested: u32,
}

impl Plan {
    /// Plan `batches` over the camera's `instances`, `tested` saying which the
    /// per-instance test judges, `clusters_of` naming the cluster table range of a
    /// batch whose mesh is split and whose run is whole clusters.
    pub(crate) fn of<'a>(
        batches: impl Iterator<Item = &'a DrawBatch>,
        (instances, tested): (&[InstanceData], &dyn Fn(usize) -> bool),
        clusters_of: impl Fn(&DrawBatch) -> Option<Range<u32>>,
    ) -> Self {
        let mut plan = Self {
            states: 1,
            ..Self::default()
        };
        for (b, batch) in batches.enumerate() {
            let Some(clusters) = clusters_of(batch) else {
                continue;
            };
            let count = batch.instances.len() as u64;
            let room = count * u64::from(batch.key.indices.len());
            let n = clusters.len() as u32;
            let fits = u64::from(plan.out_indices) + room <= MAX_OUT_INDICES
                && count <= u64::from(MAX_DIMENSION)
                && n <= MAX_DIMENSION
                && count * u64::from(n) <= 64 * u64::from(MAX_DIMENSION);
            if !fits || count == 0 {
                continue;
            }
            let first = plan.jobs.len() as u32;
            for i in batch.instances.clone() {
                let judged = tested(i as usize);
                plan.tested += if judged { n } else { 0 };
                plan.jobs.push(Job {
                    instance: i,
                    object: instances[i as usize].object,
                    tested: u32::from(judged),
                    out_first: plan.out_indices,
                    cluster_first: clusters.start,
                    state_first: plan.states,
                    cone: u32::from(batch.key.facing == Facing::Ccw),
                    _pad: 0,
                });
                plan.out_indices += batch.key.indices.len();
                plan.states += n;
            }
            let jobs = first..plan.jobs.len() as u32;
            plan.runs.push(Run {
                batch: b,
                mesh: batch.key.mesh,
                jobs,
                clusters: n,
            });
        }
        plan
    }

    /// Per opaque batch (of `batches`), the jobs whose indirect draws stand for
    /// it, or `None` for one drawn whole.
    pub(crate) fn draws(&self, batches: usize) -> Vec<Option<Range<u32>>> {
        let mut out = vec![None; batches];
        for run in &self.runs {
            out[run.batch] = Some(run.jobs.clone());
        }
        out
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
