//! What a camera's Hi-Z draws drew (#2387): the CPU records them as indirect
//! draws whose instance and index counts the GPU writes, so the triangles they
//! drew are read back from their args, a frame or so late as `hiz_culled` is,
//! and counted in `Debug.Stats`' `triangles` where the pass that draws them is.
//!
//! One read back holds the final args and the late set's after them (#2336),
//! phase one's, and the clustered batches' per-instance args (#1968), so every
//! count it gives describes one frame. Which batches each pass drew is the
//! CPU's: phase one and the late set draw the batches the depth prepass draws
//! (`BatchKey::in_prepass`), whole; the final set every opaque batch, a
//! clustered one by its kept clusters instead of its batch args.

use super::counters::{Read, Triangles};
use super::cull::ARGS;
use crate::draw::batch::OpaqueBatches;
use std::ops::Range;

/// Words in one indirect draw's args.
const WORDS: usize = (ARGS / 4) as usize;

/// Which of a frame's opaque batches each of its culled passes drew, kept
/// beside the read back of that frame's args until it is collected.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Layout {
    /// Per batch: whether the depth passes (phase one, the late set) draw it.
    prepass: Vec<bool>,
    /// Per batch: whether the final set draws it by cluster.
    clustered: Vec<bool>,
    /// The clustered draws' per-instance args.
    jobs: usize,
}

impl Layout {
    /// The layout of a frame drawing `batches`, `clustered` naming the job
    /// range of each batch drawn by cluster, over `jobs` jobs in all.
    pub(crate) fn of(
        batches: &OpaqueBatches,
        clustered: &[Option<Range<u32>>],
        jobs: usize,
    ) -> Self {
        let prepass = batches.iter().map(|b| b.key.in_prepass()).collect();
        let clustered = (0..batches.len())
            .map(|i| clustered.get(i).is_some_and(Option::is_some))
            .collect();
        Self {
            prepass,
            clustered,
            jobs,
        }
    }

    /// The read back's segments, in the order [`Self::read`] reads them: the
    /// final args and the late set's (`last`), phase one's (`first`), the
    /// cluster jobs' (`jobs`).
    pub(crate) fn segments<'a>(
        &self,
        (first, last, jobs): (&'a wgpu::Buffer, &'a wgpu::Buffer, &'a wgpu::Buffer),
    ) -> [(&'a wgpu::Buffer, u64, u64); 3] {
        let batches = ARGS * self.prepass.len() as u64;
        [
            (last, 0, 2 * batches),
            (first, 0, batches),
            (jobs, 0, ARGS * self.jobs as u64),
        ]
    }

    /// What the copied `words` say: the final set's and the late set's
    /// instances (every batch's), and the triangles each pass drew.
    pub(crate) fn read(&self, words: &[u32]) -> Read {
        let n = self.prepass.len() * WORDS;
        let at = |k: usize| words.get(k * n..(k + 1) * n).unwrap_or(&[]);
        let (last, late, first) = (at(0), at(1), at(2));
        let jobs = words.get(3 * n..).unwrap_or(&[]);
        let instances = |args: &[u32]| args.chunks_exact(WORDS).map(|a| a[1]).sum();
        let triangles = |args: &[u32], keep: &dyn Fn(usize) -> bool| -> u64 {
            let drawn = args.chunks_exact(WORDS).enumerate();
            drawn.filter(|(i, _)| keep(*i)).map(|(_, a)| of(a)).sum()
        };
        let prepass = |i: usize| self.prepass[i];
        let whole = |i: usize| !self.clustered[i];
        let jobs = &jobs[..(self.jobs * WORDS).min(jobs.len())];
        Read {
            drawn: instances(last),
            late: instances(late),
            triangles: Triangles {
                phase_one: triangles(first, &prepass),
                late: triangles(late, &prepass),
                last: triangles(last, &whole) + triangles(jobs, &|_| true),
            },
        }
    }
}

impl crate::Renderer {
    /// Count phase one's draws of `batches` (#2387): a call per batch the depth
    /// passes draw, and the `triangles` its last read back says it drew, which
    /// `hiz_depth_triangles` keeps apart.
    pub(super) fn count_phase_one(&mut self, batches: &OpaqueBatches, triangles: u64) {
        let c = &mut self.frame_counters;
        c.draw_calls += batches.iter().filter(|b| b.key.in_prepass()).count() as u32;
        c.triangles += triangles;
        c.hiz_depth_triangles += triangles;
    }
}

/// The triangles one indexed indirect draw's args draw.
fn of(args: &[u32]) -> u64 {
    u64::from(args[0] / 3) * u64::from(args[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(prepass: &[bool], clustered: &[bool], jobs: usize) -> Layout {
        Layout {
            prepass: prepass.to_vec(),
            clustered: clustered.to_vec(),
            jobs,
        }
    }

    #[test]
    fn each_pass_counts_the_batches_it_draws() {
        // Two batches, the second unlit (in no depth pass) and drawn by cluster.
        let l = layout(&[true, false], &[false, true], 2);
        let words = [
            [6, 3, 0, 0, 0, 9, 4, 0, 0, 3], // final: 3 and 4 instances
            [6, 1, 0, 0, 0, 9, 1, 0, 0, 7], // late: 1 and 1
            [6, 2, 0, 0, 0, 9, 3, 0, 0, 3], // phase one: 2 and 3
            [3, 1, 0, 0, 0, 0, 1, 0, 0, 0], // jobs: 3 indices kept, then none
        ]
        .concat();
        let read = l.read(&words);
        assert_eq!((read.drawn, read.late), (7, 2), "every batch's instances");
        let t = read.triangles;
        assert_eq!((t.phase_one, t.late), (4, 2), "the lit batch alone");
        assert_eq!(t.last, 6 + 1, "the whole batch, then the kept clusters");
    }

    #[test]
    fn a_short_or_empty_read_counts_what_it_holds() {
        let l = layout(&[true], &[false], 0);
        assert_eq!(l.read(&[]), Read::default());
        let read = l.read(&[6, 3, 0, 0, 0]);
        assert_eq!((read.drawn, read.triangles.last), (3, 6), "the final args");
    }
}
