//! Gathering a sweep's casters from the retained scene's objects (#1706): what each
//! candidate draws, its bounds and the base camera's small verdict, before
//! `cull` takes them per light volume.

use super::buffer::CasterData;
use super::cull::Candidate;
use super::{caster_facing, CasterFrame, CasterKey, ShadowRenderer, Sweep};
use crate::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::lod::LodSelection;
use crate::scene::RenderObject;
use rusty_components::ShadowCasting;

impl ShadowRenderer {
    /// Every active caster `sweep` takes — a caster a surface variant cuts counts as
    /// dynamic (see `clips`) — that has a GPU mesh, from the retained scene's
    /// objects (#1706), and the joint matrices its skinned casters' `bone_base`s
    /// index — one palette per caster per sweep, however many cascades draw it. Only
    /// what the frame decides is read here: the LOD level, the small-object verdict,
    /// and a palette object's pose and posed bounds, which animation writes unstamped.
    /// Taken of `objects`, in their order: every object, or those the sweep's
    /// volumes could reach (`RenderScene::casters_near`, #1874).
    pub(super) fn collect_casters<'a>(
        &self,
        frame: &CasterFrame,
        lod: &LodSelection,
        sweep: Sweep,
        objects: impl IntoIterator<Item = (u32, &'a RenderObject)>,
    ) -> (Vec<Candidate>, Vec<JointMatrix>) {
        let (mut casters, mut joints) = (Vec::new(), Vec::new());
        for (id, object) in objects {
            // An inactive caster, one set to cast nothing (#1571), or a level of detail
            // this sweep does not draw (#472).
            let off = !object.cast.casts() || !object.active;
            if !object.resident || off || lod.hides(id) {
                continue;
            }
            // One a camera may cull as small casts per frame, never from a bake (#1714).
            let unbaked = frame
                .objects
                .unbaked
                .holds(id, object.casting.culls_small());
            if !sweep.takes(object.is_static && !object.moving && !unbaked) {
                continue;
            }
            let world = object.world;
            // A skinned caster is bounded by its posed skeleton (#833); one whose pose
            // cannot be bounded is never culled — a wrongly-culled caster drops its shadow.
            let mesh = object.palette.then(|| frame.scene.world.mesh(id)).flatten();
            let bounds = match (&mesh, frame.gpu_mesh(object.mesh)) {
                (Some(mesh), Some(gpu)) => gpu.world_bounds(mesh, world),
                _ => object.bounds,
            };
            let small = frame.culls_small(object, bounds);
            // A culled caster never draws: no palette to upload for it.
            let palette = mesh.as_ref().map_or(&[][..], |m| m.active_palette());
            let bone_base = (!small).then(|| push_palette(&mut joints, palette));
            let bone_base = bone_base.unwrap_or(0);
            let two_sided = object.cast == ShadowCasting::TwoSided;
            for (indices, (clip, face)) in object.shadow_runs.iter().cloned() {
                let mut caster = CasterData::new(world, bone_base);
                if let Some(cut) = clip.and_then(|c| c.cutout) {
                    caster = caster.cutout(cut);
                }
                casters.push(Candidate {
                    id,
                    key: CasterKey {
                        facing: caster_facing(world, two_sided, face),
                        clip: clip.map(|c| c.clip.by_class(frame.records)),
                        mesh: object.mesh,
                        indices: indices.into(),
                    },
                    caster,
                    bounds,
                    small,
                    two_sided,
                });
            }
        }
        (casters, joints)
    }
}
