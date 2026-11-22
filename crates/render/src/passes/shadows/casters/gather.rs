use super::buffer::CasterData;
use super::cull::Candidate;
use super::{caster_facing, CasterFrame, CasterKey, ShadowRenderer, Sweep};
use crate::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::lod::LodSelection;
use crate::scene::RenderObject;
use rusty_components::ShadowCasting;

impl ShadowRenderer {
    pub(super) fn collect_casters<'a>(
        &self,
        frame: &CasterFrame,
        lod: &LodSelection,
        sweep: Sweep,
        objects: impl IntoIterator<Item = (u32, &'a RenderObject)>,
    ) -> (Vec<Candidate>, Vec<JointMatrix>) {
        let (mut casters, mut joints) = (Vec::new(), Vec::new());
        for (id, object) in objects {
            let off = !object.cast.casts() || !object.active;
            if !object.resident || off || lod.hides(id) {
                continue;
            }
            let unbaked = frame
                .objects
                .unbaked
                .holds(id, object.casting.culls_small());
            if !sweep.takes(object.is_static && !object.moving && !unbaked) {
                continue;
            }
            let world = object.world;
            let mesh = object.palette.then(|| frame.scene.world.mesh(id)).flatten();
            let bounds = match (&mesh, frame.gpu_mesh(object.mesh)) {
                (Some(mesh), Some(gpu)) => gpu.world_bounds(mesh, world),
                _ => object.bounds,
            };
            let small = frame.culls_small(object, bounds);
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
