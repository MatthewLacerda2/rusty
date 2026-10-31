//! A caster's material slots as depth draws (#1285): each slot of a multi-material
//! mesh clips its shadow by its own material, and adjacent runs that clip alike draw
//! as one; then a sweep's casters grouped into instanced runs of one key (#470).

use std::ops::Range;

use super::super::clips::CasterClip;
use super::{CasterBatch, CasterData, CasterFrame, CasterKey};
use crate::components::{MaterialComponent, RenderFace};

/// How one material slot casts: its clip (`None` casts plainly) and its material's
/// render face (#1615).
pub(super) type SlotCast<'f> = (Option<&'f CasterClip>, RenderFace);

/// Entity `id`'s `(slot, run)` draws, joined by how each slot casts. `material` is
/// its material component, borrowed once by the caller for every slot.
pub(super) fn slot_runs<'f>(
    frame: &CasterFrame<'f>,
    id: u32,
    material: Option<&MaterialComponent>,
    runs: Vec<(usize, Range<u32>)>,
) -> Vec<(Range<u32>, SlotCast<'f>)> {
    let slots = material.map_or(&[][..], |m| &m.materials[..]);
    let face = |slot: usize| {
        let key = slots.get(slot)?;
        frame.scene.materials.get(key).map(|m| m.render_face)
    };
    join_alike(runs, |slot| {
        (frame.clips.get(&(id, slot)), face(slot).unwrap_or_default())
    })
}

/// A mesh's `(slot, run)` draws ([`GpuMesh::runs`]) paired with how each slot casts
/// its shadow (`clip_of(slot)`: its clip, and its render face, #1615), adjacent runs
/// that cast alike joined into one: a mesh whose materials all cast plainly stays
/// one draw.
///
/// [`GpuMesh::runs`]: crate::render::GpuMesh::runs
pub(super) fn join_alike<T: PartialEq>(
    runs: Vec<(usize, Range<u32>)>,
    clip_of: impl Fn(usize) -> T,
) -> Vec<(Range<u32>, T)> {
    let mut out: Vec<(Range<u32>, T)> = Vec::new();
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

    #[test]
    fn plain_runs_join_into_one_draw() {
        let joined = join_alike(
            vec![(0, 0..3), (1, 3..6), (2, 6..9)],
            |_| None::<&CasterClip>,
        );
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
        let a = join_alike(runs(), |_| None::<&CasterClip>);
        let b = join_alike(runs(), |slot| (slot == 1).then_some(&leaf));
        let casters = a
            .into_iter()
            .chain(b)
            .map(|(indices, clip)| {
                let key = CasterKey {
                    facing: Facing::Ccw,
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
