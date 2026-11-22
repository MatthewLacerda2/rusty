use glam::{Mat4, Vec3};

use crate::draw::batch::sort::TransparentOrder;
use crate::draw::batch::BatchKey;
use crate::draw::map_slots::material_texture_paths;
use crate::draw::uniforms::solid_object;
use crate::frustum::transform_aabb;
use crate::gpu::pipelines::Facing;
use crate::passes::shadows::{
    depth_only, moves, shadow_runs, slot_clip, CasterClip, ObjectCasting, ShadowRun,
};
use crate::render_texture::is_render_texture;
use crate::{MeshId, Renderer, SmallLimits};
use rusty_components::{MaterialAsset, MaterialComponent, RenderFace, ShadowCasting};
use rusty_scene::Scene;

use super::handles::MeshHandle;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlotDraw {
    pub key: BatchKey,
    pub transparent: bool,
    pub priority: i32,
    pub face: RenderFace,
    pub clip: Option<CasterClip>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RenderObject {
    pub seq: u64,
    pub mesh: MeshHandle,
    pub resident: bool,
    pub local_aabb: (Vec3, Vec3),
    pub world: Mat4,
    pub bounds: Option<(Vec3, Vec3)>,
    pub palette: bool,
    pub shown: bool,
    pub active: bool,
    pub cast: ShadowCasting,
    pub moving: bool,
    pub casting: ObjectCasting,
    pub is_static: bool,
    pub layer: u8,
    pub small: SmallLimits,
    pub reads_render_texture: bool,
    pub slot: u32,
    pub slots: Vec<SlotDraw>,
    pub materials: Vec<String>,
    pub shadow_runs: Vec<ShadowRun>,
}

impl SlotDraw {
    pub(crate) fn order(&self, depth: f32) -> TransparentOrder {
        let priority = self.priority;
        TransparentOrder { priority, depth }
    }
}

impl RenderObject {
    pub(crate) fn writes_velocity(&self) -> bool {
        self.slots.iter().any(|s| {
            let t = s.key.traits;
            !s.transparent || t.writes_motion || t.depth_postpass
        })
    }

    pub(crate) fn centre(&self, bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        let (min, max) = bounds.unwrap_or_else(|| {
            let (min, max) = self.local_aabb;
            let at = self.world.transform_point3((min + max) * 0.5);
            (at, at)
        });
        (min + max) * 0.5
    }
}

fn library_names(materials: Option<&MaterialComponent>) -> Vec<String> {
    let mut names = materials.map_or_else(Vec::new, |m| m.materials.clone());
    names.sort_unstable();
    names.dedup();
    names
}

impl Renderer {
    pub(crate) fn extract_object(
        &mut self,
        scene: &Scene,
        id: u32,
        slot: Option<u32>,
    ) -> Option<RenderObject> {
        let world = &scene.world;
        let mesh = world.mesh(id)?;
        let seq = world.seq(id)?;
        let slot = slot.unwrap_or_else(|| self.retained.objects.alloc());
        let mesh_id = MeshId::from_mesh(&mesh);
        let handle = self.retained.meshes.intern(&mesh_id);
        let model = self.draw_poses.world(scene, id);
        let gpu = self.gpu_meshes.get(&mesh_id);
        let runs = gpu.map_or_else(Vec::new, |g| g.runs(&mesh, world.material(id).as_deref()));
        let local_aabb = gpu.map_or((Vec3::ZERO, Vec3::ZERO), |g| g.local_aabb);
        let palette = mesh.is_skinned() || !mesh.active_palette().is_empty();
        let active = world.is_active_in_hierarchy(id);
        let mut object = RenderObject {
            seq,
            mesh: handle,
            resident: gpu.is_some(),
            local_aabb,
            world: model,
            bounds: None,
            palette,
            shown: active && mesh.cast_shadows.is_visible(),
            active,
            cast: mesh.cast_shadows,
            moving: self.moves(scene, id),
            casting: ObjectCasting::of(scene, id, &mesh),
            is_static: world.is_static(id),
            layer: world.layer(id),
            small: SmallLimits::of(&mesh),
            reads_render_texture: false,
            slot,
            slots: Vec::with_capacity(runs.len()),
            materials: library_names(world.material(id).as_deref()),
            shadow_runs: Vec::new(),
        };
        drop(mesh);
        for run in runs {
            self.push_slot(scene, id, run, &mut object);
        }
        object.shadow_runs = shadow_runs(&object.slots);
        self.pose_object(scene, id, &mut object);
        Some(object)
    }

    fn push_slot(
        &mut self,
        scene: &Scene,
        id: u32,
        (slot, indices): (usize, std::ops::Range<u32>),
        object: &mut RenderObject,
    ) {
        let entry = scene.material_entry_at(id, slot);
        let material = entry.map(|(_, asset)| asset);
        let at = (object.world, object.mesh);
        let (mut key, transparent) = self.slot_key(scene, id, (slot, indices), at);
        let maps = material_texture_paths(material).into_iter().flatten();
        object.reads_render_texture |= maps.into_iter().any(|m| is_render_texture(m.as_str()));
        let clip = material.and_then(|m| slot_clip(&self.surface_shaders, &key, m));
        key.depth_only = depth_only(key.pipeline, clip.as_ref());
        object.slots.push(SlotDraw {
            key,
            transparent,
            priority: material.map_or(0, MaterialAsset::sort_priority),
            face: material.map_or(RenderFace::Front, |m: &MaterialAsset| m.render_face),
            clip,
        });
    }

    fn moves(&mut self, scene: &Scene, id: u32) -> bool {
        let count = scene.world.material(id).map_or(0, |m| m.materials.len());
        (0..count).any(|slot| {
            let Some((_, asset)) = scene.material_entry_at(id, slot) else {
                return false;
            };
            let shader = asset.shader.as_deref();
            let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
            moves(&self.surface_shaders, pipeline)
        })
    }

    pub(crate) fn pose_object(&mut self, scene: &Scene, id: u32, object: &mut RenderObject) {
        let model = self.draw_poses.world(scene, id);
        object.world = model;
        let (min, max) = object.local_aabb;
        let posed = object.resident && !object.palette;
        object.bounds = posed.then(|| transform_aabb(min, max, model));
        let at = (model, object.centre(object.bounds));
        let data = solid_object(scene, id, at, &self.capture, &self.lightmaps);
        self.retained.objects.write(object.slot, data);
        for slot in &mut object.slots {
            slot.key.facing = Facing::of(model, slot.face);
        }
    }
}
