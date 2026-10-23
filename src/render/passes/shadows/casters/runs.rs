//! A caster's material runs as depth draws (#1285): each run of a multi-material
//! mesh clips its shadow by its own material, and runs that clip alike draw as one.

use std::ops::Range;

use super::super::clips::CasterClip;

/// A caster's draws (`(run, clip)` per material slot, in slot order) joined for the
/// depth pass: a run adjoining the last with the same clip extends it, and one
/// the last already covers (an extra slot drawing the last run again) adds
/// nothing, since depth does not see a second pass. A mesh whose materials all cast
/// plainly stays one draw.
pub(super) fn join_runs<'c>(
    draws: impl IntoIterator<Item = (Range<u32>, Option<&'c CasterClip>)>,
) -> Vec<(Range<u32>, Option<&'c CasterClip>)> {
    let mut out: Vec<(Range<u32>, Option<&'c CasterClip>)> = Vec::new();
    for (indices, clip) in draws {
        match out.last_mut() {
            Some((last, c)) if covers(last, &indices) && same(*c, clip) => {}
            Some((last, c)) if last.end == indices.start && same(*c, clip) => {
                last.end = indices.end;
            }
            _ => out.push((indices, clip)),
        }
    }
    out
}

/// Whether `outer` holds every index of `inner`.
fn covers(outer: &Range<u32>, inner: &Range<u32>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
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
        let joined = join_runs([(0..3, None), (3..6, None), (6..9, None)]);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].0, 0..9);
    }

    #[test]
    fn a_cutout_run_draws_apart_from_its_plain_neighbours() {
        let leaf = cutout(4);
        let runs = join_runs([(0..3, None), (3..6, Some(&leaf)), (6..9, None)]);
        let shape: Vec<_> = runs.iter().map(|(r, c)| (r.clone(), c.is_some())).collect();
        assert_eq!(shape, vec![(0..3, false), (3..6, true), (6..9, false)]);
    }

    #[test]
    fn an_extra_slot_over_the_last_run_adds_no_depth_draw() {
        let joined = join_runs([(0..3, None), (3..6, None), (3..6, None)]);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].0, 0..6);
    }
}
