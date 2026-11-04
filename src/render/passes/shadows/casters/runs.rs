//! A caster's material slots as depth draws (#1285): each slot of a multi-material
//! mesh clips its shadow by its own material, and adjacent runs that clip alike draw
//! as one; then a sweep's casters grouped into instanced runs of one key (#470).

use std::ops::Range;

use super::super::clips::CasterClip;
use super::{CasterBatch, CasterData, CasterKey};
use crate::components::RenderFace;
use crate::render::scene::RenderObject;

/// How one material slot casts: its clip (`None` casts plainly) and its material's
/// render face (#1615).
pub(super) type SlotCast = (Option<CasterClip>, RenderFace);

/// `object`'s slot draws ([`GpuMesh::runs`], kept on its retained slots), joined by
/// how each slot casts.
///
/// [`GpuMesh::runs`]: crate::render::GpuMesh::runs
pub(super) fn slot_runs(object: &RenderObject) -> Vec<(Range<u32>, SlotCast)> {
    let runs = object.slots.iter();
    join_alike(runs.map(|s| (s.key.indices.range(), (s.clip, s.face))))
}

/// A mesh's runs paired with how each casts its shadow (its clip, and its render
/// face, #1615), adjacent runs that cast alike joined into one: a mesh whose
/// materials all cast plainly stays one draw.
pub(super) fn join_alike<T: PartialEq>(
    runs: impl IntoIterator<Item = (Range<u32>, T)>,
) -> Vec<(Range<u32>, T)> {
    let mut out: Vec<(Range<u32>, T)> = Vec::new();
    for (indices, clip) in runs {
        match out.last_mut() {
            Some((last, c)) if last.end == indices.start && *c == clip => {
                last.end = indices.end;
            }
            _ => out.push((indices, clip)),
        }
    }
    out
}

/// Group `casters` into instanced runs of one key, returning the packed casters (in
/// draw order) and the batches. Sorted by key — by facing, and within each plain
/// casters first, so the clipping pipelines bind last — and stable, so scene order
/// holds within a run; depth writes make the order between runs irrelevant. With `instancing` off, one draw per caster
/// in scene order (the pre-#470 path, for comparison).
pub(super) fn batch_casters(
    mut casters: Vec<(CasterKey, CasterData)>,
    instancing: bool,
) -> (Vec<CasterData>, Vec<CasterBatch>) {
    if instancing {
        casters.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let mut matrices = Vec::with_capacity(casters.len());
    let mut batches: Vec<CasterBatch> = Vec::new();
    for (key, caster) in casters {
        let index = matrices.len() as u32;
        matrices.push(caster);
        match batches.last_mut() {
            Some(run) if instancing && run.key == key => run.instances.end = index + 1,
            _ => batches.push(CasterBatch {
                key,
                instances: index..index + 1,
                record: 0,
            }),
        }
    }
    (matrices, batches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::passes::shadows::clips::{Clip, Cutout};

    fn cutout(material: usize) -> CasterClip {
        CasterClip {
            clip: Clip::of(0, material),
            cutout: Some(Cutout {
                cutoff: 0.5,
                textured: true,
                mapping: Default::default(),
            }),
        }
    }

    /// Three slots' runs of nine indices, each paired with `clip_of(slot)`.
    fn runs<T>(clip_of: impl Fn(usize) -> T) -> Vec<(Range<u32>, T)> {
        let slots = [(0, 0..3), (1, 3..6), (2, 6..9)];
        slots.into_iter().map(|(s, r)| (r, clip_of(s))).collect()
    }

    #[test]
    fn plain_runs_join_into_one_draw() {
        let joined = join_alike(runs(|_| None::<&CasterClip>));
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].0, 0..9);
    }

    #[test]
    fn a_cutout_run_draws_apart_from_its_plain_neighbours() {
        let leaf = cutout(4);
        let runs = join_alike(runs(|slot| (slot == 1).then_some(&leaf)));
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
        use crate::render::scene::MeshHandle;
        let leaf = cutout(4);
        let a = join_alike([(0..3, None), (3..6, None)]);
        let b = join_alike([(0..3, None), (3..6, Some(&leaf))]);
        let casters = a
            .into_iter()
            .chain(b)
            .map(|(indices, clip)| {
                let key = CasterKey {
                    facing: Facing::Ccw,
                    clip: clip.map(|c| c.clip),
                    mesh: MeshHandle(0),
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
