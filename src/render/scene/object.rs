//! src/render/scene/object.rs — one mesh entity as the renderer keeps it (#1706):
//! everything its draws read that does not change from frame to frame, derived once
//! by [`Renderer::extract_object`] and re-derived only when an input changed.

use glam::{Mat4, Vec3};

use crate::components::{MaterialAsset, MaterialComponent, RenderFace, ShadowCasting};
use crate::render::draw::batch::BatchKey;
use crate::render::draw::materials::material_texture_paths;
use crate::render::draw::uniforms::solid_object;
use crate::render::frustum::transform_aabb;
use crate::render::gpu::pipelines::Facing;
use crate::render::passes::shadows::{
    moves, shadow_runs, slot_clip, CasterClip, ObjectCasting, ShadowRun,
};
use crate::render::render_texture::is_render_texture;
use crate::render::{MeshId, Renderer, SmallLimits};
use crate::scene::Scene;

use super::handles::MeshHandle;

/// One material slot's draw (#1285): its batch key with `bone_base` 0 (a skinned
/// object's palette is placed per camera), whether it is transparent, the face
/// its material renders, so a pose that mirrors the object re-picks its facing, and
/// how it clips its shadow (#648; `None` casts plain depth).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlotDraw {
    pub key: BatchKey,
    pub transparent: bool,
    pub face: RenderFace,
    pub clip: Option<CasterClip>,
}

/// One mesh entity's retained draw state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RenderObject {
    /// Its insertion sequence: what orders it as `World::ids_with_mesh` does.
    pub seq: u64,
    pub mesh: MeshHandle,
    /// Its mesh's buffers are resident: without them it draws nothing.
    pub resident: bool,
    /// The resident geometry's local bounds.
    pub local_aabb: (Vec3, Vec3),
    pub world: Mat4,
    /// Its world bounds, for a mesh with no palette; `None` when unknown (not resident).
    pub bounds: Option<(Vec3, Vec3)>,
    /// It carries a bone palette, which animation writes every frame without a
    /// change stamp (`World::set_pose_palette`): its palette and posed bounds are
    /// read per frame.
    pub palette: bool,
    /// Active in the hierarchy, and not shadows-only (#1571): a camera may draw it.
    pub shown: bool,
    /// Active in the hierarchy: it may cast.
    pub active: bool,
    /// Its mesh's `cast_shadows` (#1571).
    pub cast: ShadowCasting,
    /// A surface variant whose cut moves at runtime draws one of its material slots
    /// (#648): it casts from the per-frame sweeps, even when static.
    pub moving: bool,
    /// What it adds to the static bake's key when static (`passes::shadows`).
    pub casting: ObjectCasting,
    pub is_static: bool,
    pub layer: u8,
    pub small: SmallLimits,
    /// A material map names a render texture (#430): its group's key names that
    /// texture's allocation, which the frame's views re-register.
    pub reads_render_texture: bool,
    /// Its slot in the renderer's object table (`table.rs`), which holds its
    /// `ObjectData` (world matrix, probe SH, lightmap): kept across re-derives,
    /// freed when the object goes. A camera's instance names it.
    pub slot: u32,
    pub slots: Vec<SlotDraw>,
    /// The library names its material slots reference, found or not, each once
    /// (sorted): an edit to one of them re-derives it (`RenderScene::by_material`),
    /// and no other does.
    pub materials: Vec<String>,
    /// Its slots' depth draws, adjacent slots that cast alike joined
    /// (`passes::shadows::shadow_runs`): derived with the slots, so a shadow sweep
    /// allocates nothing per caster.
    pub shadow_runs: Vec<ShadowRun>,
}

impl RenderObject {
    /// Whether any slot writes velocity: an opaque one (#1435), or a translucent
    /// one writing its motion or its depth postpass (#1845, #1523). Only those need
    /// a last-frame pose; without one a translucent's texel moved with the camera
    /// alone.
    pub(crate) fn writes_velocity(&self) -> bool {
        self.slots.iter().any(|s| {
            let t = s.key.traits;
            !s.transparent || t.writes_motion || t.depth_postpass
        })
    }

    /// Its world bounds' centre, Unity's `Renderer.bounds.center` (#1993, #1994):
    /// where both sorts, the reflection-probe pick and the light-probe sample place
    /// it, so a mesh modelled in world coordinates (its origin far from its geometry)
    /// is placed where it stands. `bounds` is its world bounds now (a skinned one's
    /// posed bound); without them, its local bounds' centre under its world matrix,
    /// which is its origin while its mesh is not resident.
    pub(crate) fn centre(&self, bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        let (min, max) = bounds.unwrap_or_else(|| {
            let (min, max) = self.local_aabb;
            let at = self.world.transform_point3((min + max) * 0.5);
            (at, at)
        });
        (min + max) * 0.5
    }
}

/// The names `materials` references, each once: two slots may name one material
/// (`scene::authoring::material::set_slot` pads with its key), and `RenderScene::set` indexes
/// each name once.
fn library_names(materials: Option<&MaterialComponent>) -> Vec<String> {
    let mut names = materials.map_or_else(Vec::new, |m| m.materials.clone());
    names.sort_unstable();
    names.dedup();
    names
}

impl Renderer {
    /// Entity `id` as the renderer keeps it, `None` when it has no mesh. Reads every
    /// input a draw of it takes but the per-camera ones; its mesh must already be
    /// uploaded and its maps loaded, so the keys it builds bind what is resident. Its
    /// record goes into object-table slot `slot` (a fresh one when `None`).
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

    /// Add material slot `slot` (drawing `indices`) of entity `id` to `object`: its
    /// key, whether it is transparent, its face and its shadow clip.
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
        let (key, transparent) = self.slot_key(scene, id, (slot, indices), at);
        let maps = material_texture_paths(material).into_iter().flatten();
        object.reads_render_texture |= maps.into_iter().any(|m| is_render_texture(m.as_str()));
        let clip = material.and_then(|m| slot_clip(&self.surface_shaders, &key, m));
        object.slots.push(SlotDraw {
            key,
            transparent,
            face: material.map_or(RenderFace::Front, |m: &MaterialAsset| m.render_face),
            clip,
        });
    }

    /// Whether a surface variant whose cut moves cuts any of entity `id`'s material
    /// slots, drawn or not, as the per-frame sweeps have always read it.
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

    /// Re-place `object` (entity `id`) under its world matrix now: its bounds, its
    /// record in the object table (the probe SH it samples at its bounds' centre, its
    /// lightmap) and each slot's facing. All a move changes; what else it changes is
    /// a full extract's.
    pub(crate) fn pose_object(&mut self, scene: &Scene, id: u32, object: &mut RenderObject) {
        let model = self.draw_poses.world(scene, id); // blended between ticks (#1877)
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
