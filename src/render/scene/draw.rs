//! src/render/scene/draw.rs — one camera's solids from the retained objects (#1706):
//! cull each object by what the camera decides, then turn it into draw items. Only
//! the per-camera inputs (`inputs.rs`) are computed here; the rest is the object's.

use glam::{Mat4, Vec3};

use super::RenderObject;
use crate::render::draw::batch::sort::view_depth;
use crate::render::draw::batch::DrawItem;
use crate::render::draw::proxy_volume;
use crate::render::draw::visibility::{CameraCulls, ViewCull};
use crate::render::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::render::motion::{track, MotionHistory};
use crate::render::{InstanceData, Renderer};
use crate::scene::Scene;

/// A mesh's world bounds, `None` when they cannot be known (never culled).
type Bounds = Option<(Vec3, Vec3)>;

/// What one camera collects: its opaque items, its transparent items with their
/// view depth, and the joint palettes and proxy volumes they index.
pub(crate) struct CameraSolids<'a> {
    pub opaque: &'a mut Vec<DrawItem>,
    pub transparent: &'a mut Vec<(DrawItem, f32)>,
    pub joints: &'a mut Vec<JointMatrix>,
    pub proxies: &'a mut proxy_volume::Frame,
}

impl Renderer {
    /// Collect `scene`'s retained solids `view` draws into `out`, each marked for the
    /// outline when `ringed` names it; with `history`, each carries its last-frame
    /// pose (#1435). An object the frustum and the small-object cull kept is then
    /// left out when the scene's baked PVS hides it from the camera's cell (#1714).
    /// Returns what culling skipped and how many objects drew.
    pub(crate) fn camera_solids(
        &mut self,
        scene: &Scene,
        (view, eye, ringed): (&ViewCull, (Vec3, Vec3), &[u32]),
        mut out: CameraSolids,
        mut history: Option<&mut MotionHistory>,
    ) -> (CameraCulls, u32) {
        if self.retained.scene(scene.id()).is_none() {
            self.extract_scene(scene); // drawn with no frame prepared (a test)
        }
        let retained = self.retained.take(scene.id());
        let (mut culls, mut visible) = (CameraCulls::default(), 0);
        let pvs = &retained.occlusion;
        let row = view.occlusion.and_then(|eye| pvs.row(eye, view.mask));
        for (id, object) in retained.objects() {
            let Some(bounds) = self.cull_object(scene, (id, object), view, &mut culls) else {
                continue;
            };
            // The selection outline rings a solid through walls: a ringed one stays.
            if row.is_some_and(|row| pvs.hides(row, id)) && !ringed.contains(&id) {
                culls.occluded += 1;
                continue;
            }
            if !object.resident || object.slots.is_empty() {
                continue;
            }
            visible += 1;
            let at = (id, object, bounds);
            let outline = ringed.contains(&id);
            self.object_items(scene, at, (eye, outline), &mut history, &mut out);
        }
        self.keep_scene(scene.id(), retained);
        (culls, visible)
    }

    /// `object`'s world bounds when `view` draws it, its verdict counted in `culls`
    /// when it doesn't: its mask or a capture leaves it out, then its LOD level is
    /// hidden (#472), then the frustum (#330), then too far or too small on what the
    /// frustum kept (#1714). Whichever culls first counts.
    fn cull_object(
        &self,
        scene: &Scene,
        (id, object): (u32, &RenderObject),
        view: &ViewCull,
        culls: &mut CameraCulls,
    ) -> Option<Bounds> {
        // An inactive mesh, or a shadows-only one: it casts, no camera draws it (#1571).
        // A bake's capture gathers static geometry alone (#243).
        let static_only = self.capture.static_only && !object.is_static;
        if !object.shown || static_only || !crate::scene::layer_in_mask(object.layer, view.mask) {
            return None;
        }
        if view.lod.hides(id) {
            culls.lod_hidden += 1;
            return None;
        }
        culls.tested += 1;
        let bounds = self.object_bounds(scene, id, object);
        if bounds.is_some_and(|(min, max)| !view.frustum.intersects_aabb(min, max)) {
            culls.culled += 1;
            return None;
        }
        let small = |(min, max): (Vec3, Vec3)| {
            let centre = (min + max) * 0.5;
            view.small
                .culls_by(object.small, centre, object.local_aabb, object.world)
        };
        if bounds.is_some_and(small) {
            culls.size_culled += 1;
            return None;
        }
        Some(bounds)
    }

    /// `object`'s world bounds: kept, or for one with a palette its posed bound now
    /// (#833). `None` when its mesh is not resident.
    fn object_bounds(&self, scene: &Scene, id: u32, object: &RenderObject) -> Bounds {
        if !object.palette {
            return object.bounds;
        }
        let mesh = scene.world.mesh(id)?;
        self.gpu_mesh_of(object.mesh)?
            .world_bounds(&mesh, object.world)
    }

    /// `object`'s draw items, one per material slot (#1285), into `out`: an instance
    /// of its object-table slot with the reflection probe picked at its bounds' centre (#1206) and its proxy
    /// volume (#1672), its palette appended to the joints (a skinned one is a draw of
    /// its own), and with `history` its last-frame pose.
    fn object_items(
        &mut self,
        scene: &Scene,
        (id, object, bounds): (u32, &RenderObject, Bounds),
        (eye, outline): ((Vec3, Vec3), bool),
        history: &mut Option<&mut MotionHistory>,
        out: &mut CameraSolids,
    ) {
        let mesh = object.palette.then(|| scene.world.mesh(id)).flatten();
        let palette: &[Mat4] = mesh.as_ref().map_or(&[], |m| m.active_palette());
        let origin = object.origin();
        let centre = bounds.map_or(origin, |(min, max)| (min + max) * 0.5);
        let mut instance = InstanceData::of(object.slot);
        instance.reflection_probe = self.reflection_probes.probe_for(centre);
        // A capture draws static geometry for a bake: its probes are in flux.
        let use_sh = self.retained.objects.get(object.slot).use_sh == 1;
        if use_sh && !self.capture.static_only {
            let at = (object.world, object.local_aabb);
            instance.proxy_volume = self.proxy_volumes.push(scene, id, at, out.proxies);
        }
        let bone_base = push_palette(out.joints, palette);
        if let Some(history) = history.as_deref_mut().filter(|_| object.has_opaque_run()) {
            let pose = (object.world, palette);
            track(history, id, pose, out.joints, &mut instance);
        }
        let depth = view_depth(centre, eye.0, eye.1);
        for slot in &object.slots {
            let item = DrawItem {
                key: crate::render::draw::batch::BatchKey {
                    bone_base,
                    ..slot.key
                },
                instance,
                depth,
                outline,
            };
            match slot.transparent {
                true => out
                    .transparent
                    .push((item, view_depth(origin, eye.0, eye.1))),
                false => out.opaque.push(item),
            }
        }
    }
}
