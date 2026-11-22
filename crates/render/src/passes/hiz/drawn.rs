use super::counters::{Read, Triangles};
use super::cull::ARGS;
use crate::draw::batch::OpaqueBatches;
use std::ops::Range;

const WORDS: usize = (ARGS / 4) as usize;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Layout {
    prepass: Vec<bool>,
    clustered: Vec<bool>,
    jobs: usize,
}

impl Layout {
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
    pub(super) fn count_phase_one(&mut self, batches: &OpaqueBatches, read: Option<u64>) {
        let depth = batches.iter().filter(|b| b.key.in_prepass());
        let draws = depth.map(|b| (b.key.indices.len(), b.instances.len() as u32));
        let c = &mut self.frame_counters;
        c.hiz_depth_triangles += c.add_read_draws(draws, read);
    }
}

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
        let l = layout(&[true, false], &[false, true], 2);
        let words = [
            [6, 3, 0, 0, 0, 9, 4, 0, 0, 3],
            [6, 1, 0, 0, 0, 9, 1, 0, 0, 7],
            [6, 2, 0, 0, 0, 9, 3, 0, 0, 3],
            [3, 1, 0, 0, 0, 0, 1, 0, 0, 0],
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
