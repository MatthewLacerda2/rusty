use std::ops::Range;

use crate::draw::batch::DrawBatch;
use crate::gpu::pipelines::winding::Facing;
use crate::InstanceData;

/// Output indices, `u32`s: equal to the import's largest run.
pub(crate) const MAX_OUT_INDICES: u64 = rusty_asset::clusters::MAX_RUN_INDICES as u64;
/// Clusters a chunk covers: `hiz_clusters.wgsl`'s workgroup size and its `64u`s.
pub(crate) const CHUNK: u32 = 64;

/// Mirrors `hiz_clusters.wgsl`'s `Job`: change the two together.
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
    pub clusters: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    pub batch: usize,
    pub jobs: Range<u32>,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Plan {
    pub jobs: Vec<Job>,
    pub runs: Vec<Run>,
    pub chunks: Vec<[u32; 2]>,
    pub out_indices: u32,
    pub states: u32,
    pub tested: u32,
}

impl Plan {
    pub(crate) fn of<'a>(
        batches: impl Iterator<Item = &'a DrawBatch>,
        (instances, tested): (&[InstanceData], &dyn Fn(usize) -> bool),
        clusters_of: impl Fn(&DrawBatch) -> Option<Range<u32>>,
    ) -> Self {
        let mut plan = Self::default();
        for (b, batch) in batches.enumerate() {
            let Some(clusters) = clusters_of(batch) else {
                continue;
            };
            let count = batch.instances.len() as u64;
            let room = count * u64::from(batch.key.indices.len());
            let fits = u64::from(plan.out_indices) + room <= MAX_OUT_INDICES;
            let judged = batch.instances.clone().any(|i| tested(i as usize));
            // Unjudged, it would cull only by frustum and cone, a draw an instance: one draw wins.
            if !fits || !judged {
                continue;
            }
            let n = clusters.len() as u32;
            let first = plan.jobs.len() as u32;
            for i in batch.instances.clone() {
                let judged = tested(i as usize);
                plan.tested += if judged { n } else { 0 };
                let job = plan.jobs.len() as u32;
                plan.chunks
                    .extend((0..n).step_by(CHUNK as usize).map(|k| [job, k]));
                plan.jobs.push(Job {
                    instance: i,
                    object: instances[i as usize].object,
                    tested: u32::from(judged),
                    out_first: plan.out_indices,
                    cluster_first: clusters.start,
                    state_first: plan.states,
                    cone: u32::from(batch.key.facing == Facing::Ccw),
                    clusters: n,
                });
                plan.out_indices += batch.key.indices.len();
                // State words: the kept count, then two a cluster.
                plan.states += 1 + 2 * n;
            }
            let jobs = first..plan.jobs.len() as u32;
            plan.runs.push(Run { batch: b, jobs });
        }
        plan
    }

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
