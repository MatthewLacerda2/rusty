//! A caster's material slots as depth draws (#1285): each slot of a multi-material
//! mesh clips its shadow by its own material, and adjacent runs that clip alike draw
//! as one.

use std::ops::Range;

use super::super::clips::CasterClip;

/// A mesh's `(slot, run)` draws ([`GpuMesh::runs`]) paired with how each slot clips
/// its shadow (`clip_of(slot)`), adjacent runs that clip alike joined into one: a mesh
/// whose materials all cast plainly stays one draw.
///
/// [`GpuMesh::runs`]: crate::render::GpuMesh::runs
pub(super) fn join_alike<'c>(
    runs: Vec<(usize, Range<u32>)>,
    clip_of: impl Fn(usize) -> Option<&'c CasterClip>,
) -> Vec<(Range<u32>, Option<&'c CasterClip>)> {
    let mut out: Vec<(Range<u32>, Option<&'c CasterClip>)> = Vec::new();
    for (slot, indices) in runs {
        let clip = clip_of(slot);
        match out.last_mut() {
            Some((last, c)) if last.end == indices.start && *c == clip => {
                last.end = indices.end;
            }
            _ => out.push((indices, clip)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::passes::shadows::clips::{Clip, Cutout};

    fn cutout(material: usize) -> CasterClip {
        CasterClip {
            clip: Clip {
                pipeline: 0,
                material,
            },
            cutout: Some(Cutout {
                cutoff: 0.5,
                textured: true,
                mapping: Default::default(),
            }),
        }
    }

    #[test]
    fn plain_runs_join_into_one_draw() {
        let joined = join_alike(vec![(0, 0..3), (1, 3..6), (2, 6..9)], |_| None);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].0, 0..9);
    }

    #[test]
    fn a_cutout_run_draws_apart_from_its_plain_neighbours() {
        let leaf = cutout(4);
        let runs = join_alike(vec![(0, 0..3), (1, 3..6), (2, 6..9)], |slot| {
            (slot == 1).then_some(&leaf)
        });
        let shape: Vec<_> = runs.iter().map(|(r, c)| (r.clone(), c.is_some())).collect();
        assert_eq!(shape, vec![(0..3, false), (3..6, true), (6..9, false)]);
    }

    /// Two entities on one mesh, one whose runs all cast plainly (joined into one
    /// draw) and one whose second run is Cutout: each casts its own shape, so the
    /// plain halves never merge into one instanced draw of the wrong run.
    #[test]
    fn differently_joined_runs_of_one_mesh_never_share_a_draw() {
        use super::super::{batch_casters, CasterData, CasterKey};
        use crate::render::gpu::pipelines::winding::Facing;
        use crate::render::MeshId;
        let leaf = cutout(4);
        let runs = || vec![(0, 0..3), (1, 3..6)];
        let a = join_alike(runs(), |_| None);
        let b = join_alike(runs(), |slot| (slot == 1).then_some(&leaf));
        let casters = a
            .into_iter()
            .chain(b)
            .map(|(indices, clip)| {
                let key = CasterKey {
                    facing: Facing::Front,
                    clip: clip.map(|c| c.clip),
                    mesh: MeshId("Quad".into()),
                    indices: indices.into(),
                };
                (key, CasterData::new(glam::Mat4::IDENTITY, 0))
            })
            .collect();
        let (_, batches) = batch_casters(casters, true);
        let drawn: Vec<_> = batches
            .iter()
            .map(|b| (b.key.clip.is_some(), b.key.indices.range(), b.counts().1))
            .collect();
        assert_eq!(drawn, [(false, 0..3, 1), (false, 0..6, 1), (true, 3..6, 1)]);
    }
}
