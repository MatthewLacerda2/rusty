use glam::{Mat4, Vec3};

use super::RenderObject;
use crate::draw::batch::sort::{view_depth, TransparentOrder};
use crate::draw::batch::DrawItem;
use crate::draw::proxy_volume;
use crate::draw::visibility::{CameraCulls, ViewCull};
use crate::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::motion::{track, MotionHistory};
use crate::{InstanceData, Renderer};
use rusty_scene::Scene;

/// A mesh's world bounds, `None` when they cannot be known (never culled).
type Bounds = Option<(Vec3, Vec3)>;

pub(crate) struct CameraSolids<'a> {
    pub opaque: &'a mut Vec<DrawItem>,
    pub transparent: &'a mut Vec<(DrawItem, TransparentOrder)>,
    pub joints: &'a mut Vec<JointMatrix>,
    pub proxies: &'a mut proxy_volume::Frame,
    pub tested: Option<&'a mut Vec<(u32, (Vec3, Vec3))>>,
}

fn hiz_testable(object: &RenderObject, outline: bool) -> bool {
    !outline && !object.moving
}

impl Renderer {
    pub(crate) fn camera_solids(
        &mut self,
        scene: &Scene,
        (view, eye, ringed): (&ViewCull, (Vec3, Vec3), &[u32]),
        mut out: CameraSolids,
        mut history: Option<&mut MotionHistory>,
    ) -> (CameraCulls, u32) {
        if self.retained.scene(scene.id()).is_none() {
            self.extract_scene(scene);
        }
        let retained = self.retained.take(scene.id());
        let (mut culls, mut visible) = (CameraCulls::default(), 0);
        let streams = self.streams_textures();
        let pvs = &retained.occlusion;
        let row = view.occlusion.and_then(|eye| pvs.row(eye, view.mask));
        for (id, object) in retained.objects() {
            let Some(bounds) = self.cull_object(scene, (id, object), view, &mut culls) else {
                continue;
            };
            if row.is_some_and(|row| pvs.hides(row, id)) && !ringed.contains(&id) {
                culls.occluded += 1;
                continue;
            }
            if !object.resident || object.slots.is_empty() {
                continue;
            }
            visible += 1;
            if let Some((min, max)) = bounds.filter(|_| streams) {
                self.see_slots(
                    object,
                    view.small
                        .share((min + max) * 0.5, (max - min).max_element()),
                );
            }
            let at = (id, object, bounds);
            let outline = ringed.contains(&id);
            let testable = bounds.filter(|_| hiz_testable(object, outline));
            if let (Some(tested), Some(bounds)) = (out.tested.as_mut(), testable) {
                tested.push((object.slot, bounds));
            }
            self.object_items(scene, at, (eye, outline), &mut history, &mut out);
        }
        self.keep_scene(scene.id(), retained);
        (culls, visible)
    }

    fn see_slots(&mut self, object: &RenderObject, share: Option<f32>) {
        let Some(share) = share else {
            return;
        };
        for slot in &object.slots {
            self.streaming.see(slot.key.material, share);
        }
    }

    fn cull_object(
        &self,
        scene: &Scene,
        (id, object): (u32, &RenderObject),
        view: &ViewCull,
        culls: &mut CameraCulls,
    ) -> Option<Bounds> {
        let static_only = self.capture.static_only && !object.is_static;
        if !object.shown || static_only || !rusty_scene::layer_in_mask(object.layer, view.mask) {
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

    fn object_bounds(&self, scene: &Scene, id: u32, object: &RenderObject) -> Bounds {
        if !object.palette {
            return object.bounds;
        }
        let mesh = scene.world.mesh(id)?;
        self.gpu_mesh_of(object.mesh)?
            .world_bounds(&mesh, object.world)
    }

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
        let centre = object.centre(bounds);
        let mut instance = InstanceData::of(object.slot);
        instance.reflection_probe = self.reflection_probes.probe_for(centre);
        let use_sh = self.retained.objects.get(object.slot).use_sh == 1;
        // A capture bakes the probes a proxy volume samples, so it binds none.
        if use_sh && !self.capture.static_only {
            let at = (object.world, object.local_aabb);
            instance.proxy_volume = self.proxy_volumes.push(scene, id, at, out.proxies);
        }
        let bone_base = push_palette(out.joints, palette);
        if let Some(history) = history.as_deref_mut().filter(|_| object.writes_velocity()) {
            let pose = (object.world, palette);
            track(history, id, pose, out.joints, &mut instance);
        }
        let depth = view_depth(centre, eye.0, eye.1);
        for slot in &object.slots {
            let item = DrawItem {
                key: crate::draw::batch::BatchKey {
                    bone_base,
                    ..slot.key
                },
                instance,
                depth,
                outline,
            };
            match slot.transparent {
                true => out.transparent.push((item, slot.order(depth))),
                false => out.opaque.push(item),
            }
        }
    }
}
