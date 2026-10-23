//! A caster's material runs as depth draws (#1285): each run of a multi-material
//! mesh clips its shadow by its own material, and runs that clip alike draw as one.

use std::ops::Range;

use super::super::clips::CasterClip;

/// `runs` (a mesh's material runs, in order) paired with how each clips its shadow
/// (`clip_of(run)`), adjacent runs that clip alike joined into one: a mesh whose
/// materials all cast plainly stays one draw.
pub(super) fn clip_runs<'c>(
    runs: Vec<Range<u32>>,
    clip_of: impl Fn(usize) -> Option<&'c CasterClip>,
) -> Vec<(Range<u32>, Option<&'c CasterClip>)> {
    let mut out: Vec<(Range<u32>, Option<&'c CasterClip>)> = Vec::new();
    for (run, indices) in runs.into_iter().enumerate() {
        let clip = clip_of(run);
        match out.last_mut() {
            Some((last, c)) if last.end == indices.start && same(*c, clip) => {
                last.end = indices.end;
            }
            _ => out.push((indices, clip)),
        }
    }
    out
}

/// Whether two runs clip their shadow the same way (or both not at all).
fn same(a: Option<&CasterClip>, b: Option<&CasterClip>) -> bool {
    a.map(|c| c.clip) == b.map(|c| c.clip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::passes::shadows::clips::Clip;

    fn cutout(material: usize) -> CasterClip {
        CasterClip {
            clip: Clip {
                pipeline: 0,
                material,
            },
            cutout: Some((0.5, true)),
        }
    }

    #[test]
    fn plain_runs_join_into_one_draw() {
        let joined = clip_runs(vec![0..3, 3..6, 6..9], |_| None);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].0, 0..9);
    }

    #[test]
    fn a_cutout_run_draws_apart_from_its_plain_neighbours() {
        let leaf = cutout(4);
        let runs = clip_runs(vec![0..3, 3..6, 6..9], |run| (run == 1).then_some(&leaf));
        let shape: Vec<_> = runs.iter().map(|(r, c)| (r.clone(), c.is_some())).collect();
        assert_eq!(shape, vec![(0..3, false), (3..6, true), (6..9, false)]);
    }
}
