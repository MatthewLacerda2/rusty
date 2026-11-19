//! Which of a camera's opaque batches are drawn by cluster this frame (#1968),
//! and the jobs `hiz_clusters.wgsl` runs for them: one per instance, each with
//! its room in the output index list, and the dispatch groups that run them:
//! the batches of one mesh share a dispatch a stage while the padding stays
//! bounded (#2406). Pure CPU work over the frame's batches.

use std::ops::Range;

use crate::draw::batch::DrawBatch;
use crate::gpu::pipelines::winding::Facing;
use crate::scene::MeshHandle;
use crate::InstanceData;

/// The most output indices one camera's clustered draws may hold (32 MB): a
/// batch that would pass it is drawn whole, by the per-instance test alone. The
/// import splits no run longer than it, since one could never fit.
pub(crate) const MAX_OUT_INDICES: u64 = rusty_asset::clusters::MAX_RUN_INDICES as u64;
/// A dispatch dimension's limit, in workgroups: a batch with more instances or
/// clusters than this, or more (instance, cluster) pairs than 64 times it (the
/// test's threads), is drawn whole.
const MAX_DIMENSION: u32 = 65535;
/// How many times its real (instance, cluster) pairs a group's dispatch rows may
/// start: a group's jobs each take a test row and a copy row as wide as its
/// widest batch, so joining batches of unequal counts pads the narrower ones.
/// At 2, a group pads at most as much as it works: batches of 40 and 60 clusters
/// join, a 2-cluster batch of 1,000 instances and a 500-cluster one do not.
pub(crate) const MAX_PADDING: u64 = 2;

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
    /// Its batch's clusters: its group's dispatch covers the most of any job in
    /// it, so a job of a smaller batch skips the rest.
    pub clusters: u32,
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

/// The clustered batches one dispatch a stage covers: consecutive runs of one
/// mesh, so one mesh binding (group 1) serves them all, their jobs contiguous;
/// `clusters`, the most any of them has, is every job's row (the test's threads,
/// the copy's 64-lane workgroups at most), `pairs` the rows' real work, and
/// `jobs × clusters ≤ MAX_PADDING × pairs`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Group {
    pub mesh: MeshHandle,
    pub jobs: Range<u32>,
    pub clusters: u32,
    pub pairs: u64,
}

/// A camera's clustered draws this frame.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Plan {
    pub jobs: Vec<Job>,
    pub runs: Vec<Run>,
    /// The runs' dispatch groups, in order: each stage dispatches once a group.
    pub groups: Vec<Group>,
    /// Output indices and state words the jobs need: a job's region is
    /// `1 + 2 * clusters` words (`hiz_clusters.wgsl`'s `state`).
    pub out_indices: u32,
    pub states: u32,
    /// The clusters of instances the per-instance test judges: `Debug.Stats`'
    /// `cluster_tested`.
    pub tested: u32,
}

impl Plan {
    /// Plan `batches` over the camera's `instances`, `tested` saying which the
    /// per-instance test judges, `clusters_of` naming the cluster table range of a
    /// batch whose mesh is split and whose run is whole clusters. A batch with no
    /// instance, or none the instance test judges, keeps its one instanced draw.
    pub(crate) fn of<'a>(
        batches: impl Iterator<Item = &'a DrawBatch>,
        (instances, tested): (&[InstanceData], &dyn Fn(usize) -> bool),
        clusters_of: impl Fn(&DrawBatch) -> Option<Range<u32>>,
    ) -> Self {
        let mut plan = Self::default();
        // By mesh, cluster count and index run, so a mesh's batches are
        // consecutive, nearest counts beside each other (a greedy join then makes
        // few groups), and copies of one range (differing in material, facing or
        // two-sidedness) side by side; stable, so the order stays a function of
        // the batches alone.
        let mut batches: Vec<_> = batches
            .enumerate()
            .filter_map(|(b, batch)| Some((b, batch, clusters_of(batch)?)))
            .collect();
        batches.sort_by_key(|(_, b, c)| (b.key.mesh, c.len(), b.key.indices));
        for (b, batch, clusters) in batches {
            let count = batch.instances.len() as u64;
            let room = count * u64::from(batch.key.indices.len());
            let n = clusters.len() as u32;
            let fits = u64::from(plan.out_indices) + room <= MAX_OUT_INDICES
                && n <= MAX_DIMENSION
                && fits_dispatch(count, n);
            // A batch none of whose instances is judged (mesh particles, a
            // vertex-moving material) would cull nothing by cluster but the
            // frustum and cones, at a draw per instance: one instanced draw wins.
            let judged = batch.instances.clone().any(|i| tested(i as usize));
            if !fits || !judged {
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
                    clusters: n,
                });
                plan.out_indices += batch.key.indices.len();
                plan.states += 1 + 2 * n;
            }
            let jobs = first..plan.jobs.len() as u32;
            plan.group(batch.key.mesh, jobs.clone(), n);
            plan.runs.push(Run {
                batch: b,
                mesh: batch.key.mesh,
                jobs,
                clusters: n,
            });
        }
        plan
    }

    /// Put a run of `mesh` (its `jobs`, `n` clusters each) in the last group when
    /// that is of the same mesh, the joined dispatches stay in their limits and
    /// its rows stay within `MAX_PADDING` times its real pairs, else in a group of
    /// its own. Every job of a group starts a row as wide as its widest batch, so
    /// an unbounded join of a 2-cluster batch of 1,000 instances and a 500-cluster
    /// one would start 500,000 copy workgroups for at most 2,500 clusters, while
    /// a dispatch per batch pays a fixed cost a dispatch (on a real level, the
    /// larger one).
    fn group(&mut self, mesh: MeshHandle, jobs: Range<u32>, n: u32) {
        let pairs = u64::from(jobs.end - jobs.start) * u64::from(n);
        let joined = self.groups.last_mut().filter(|g| {
            let count = u64::from(jobs.end - g.jobs.start);
            let widest = g.clusters.max(n);
            g.mesh == mesh
                && fits_dispatch(count, widest)
                && count * u64::from(widest) <= MAX_PADDING * (g.pairs + pairs)
        });
        match joined {
            Some(g) => {
                g.jobs.end = jobs.end;
                g.clusters = g.clusters.max(n);
                g.pairs += pairs;
            }
            None => self.groups.push(Group {
                mesh,
                jobs,
                clusters: n,
                pairs,
            }),
        }
    }

    /// The (instance, cluster) rows the groups' dispatches start, and the real
    /// pairs among them: the padding the join chose.
    #[cfg(test)]
    pub(crate) fn rows_and_pairs(&self) -> (u64, u64) {
        let rows = |g: &Group| u64::from(g.jobs.end - g.jobs.start) * u64::from(g.clusters);
        let rows = self.groups.iter().map(rows).sum();
        (rows, self.groups.iter().map(|g| g.pairs).sum())
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

/// Whether `jobs` instances of `clusters` each fit one dispatch a stage: the
/// scan's and copy's workgroup a job, the test's thread an (instance, cluster).
fn fits_dispatch(jobs: u64, clusters: u32) -> bool {
    jobs <= u64::from(MAX_DIMENSION) && jobs * u64::from(clusters) <= 64 * u64::from(MAX_DIMENSION)
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
