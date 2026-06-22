//! src/scene/scene.rs — Scene: the hecs-backed `ecs::World` plus scene-level state.
//!
//! Entity/component storage of record is `ecs::World` (one `Entity` bundle per
//! hecs entity, generational handles + stable-id/name lookup). `Scene` owns that
//! `World` together with the engine-level scene scalars (selection, skybox,
//! ambient light) and the parenting / collider / serialization helpers that
//! operate across the World — concerns that must NOT live inside `ecs::World`
//! (whose deps are restricted to hecs + components). This wrapper lives in the
//! `scene` layer, which is allowed to depend on `crate::scene` and rendering.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};

use crate::ecs::world::{Ref, RefMut};
use crate::ecs::World;
use crate::scene::collision_matrix::CollisionMatrix;
use crate::scene::layers::LayerRegistry;

// Re-export the component types so the many existing `scene::…Component` paths
// resolve. The structs themselves live in `crate::components`.
pub use crate::components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ClearFlags, ColliderComponent,
    ColliderShape, CollisionResponse, DirtyFlag, EmitMode, Entity, HealthComponent, LightComponent,
    LightType, MaterialAsset, MaterialComponent, MeshComponent, NavMeshAgentComponent, Particle,
    ParticleBlend, ParticleEmitterComponent, RigidBodyComponent, ScriptComponent, ScriptFieldValue,
    TextureComponent, Tonemap, TransformComponent, VisualCorrectionComponent,
};

fn default_skybox_path() -> String {
    "".to_string()
}

/// The default hemisphere-ambient SKY colour (the up-facing tint) — a soft daylight
/// blue. This is the engine's single source of truth for the default ambient: the
/// serde defaults in `serialize.rs` re-export it, and the procedural gradient sky
/// (#256) derives its dome from the same `lighting.ambient` uniform the surface
/// shader reads, so background and lighting always agree. The ground term is this
/// colour scaled down in the shader. Bright/coloured enough that two differently
/// oriented faces visibly differ with NO baked probes and without a direct light on
/// every face.
pub const DEFAULT_AMBIENT_COLOR: Vec3 = Vec3::new(0.45, 0.55, 0.70);
/// The default hemisphere-ambient intensity (scales [`DEFAULT_AMBIENT_COLOR`]).
pub const DEFAULT_AMBIENT_INTENSITY: f32 = 0.55;

fn default_ambient_color() -> Vec3 {
    DEFAULT_AMBIENT_COLOR
}
fn default_ambient_intensity() -> f32 {
    DEFAULT_AMBIENT_INTENSITY
}

pub struct Scene {
    pub world: World,
    pub selected_entity_id: Option<u32>,
    pub skybox_path: String,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    /// The project's shared Layers registry (Unity's Tags & Layers). Serialized
    /// with the scene; the per-entity `layer` index points into it.
    pub layers: LayerRegistry,
    /// Unity's Layer Collision Matrix: which layer collides with which. Drives
    /// rapier `InteractionGroups` on collider build (#91); serialized with the scene.
    pub collision_matrix: CollisionMatrix,
    /// The per-World material library: reusable `MaterialAsset`s keyed by name.
    /// Entities reference a material by name via their `MaterialComponent`; many
    /// entities can share one entry. A `BTreeMap` (not `HashMap`) so serialization
    /// order is deterministic (the repo has a determinism guard).
    pub materials: BTreeMap<String, MaterialAsset>,
    /// Runtime box-projector decals (bullet holes, scorch, blood splats). These are
    /// ephemeral *visual* state spawned from raycast hits, NOT serialized scene
    /// data — the decal renderer reads them each frame and projects them onto the
    /// surfaces they overlap. Bounded FIFO (oldest evicted past `MAX_DECALS`).
    pub decals: Vec<crate::render::decals::Decal>,
    /// Scene-level light-probe dataset (#240): probe POSITIONS + grid layout live in
    /// the scene document; their baked L2 SH irradiance lives in the
    /// `<scene>.lighting.json` sidecar. Dynamic (non-static) objects sample the
    /// interpolated probe field instead of the flat hemispherical ambient term.
    pub probes: crate::scene::probe::ProbeVolume,
    /// Scene-level reflection-probe dataset (#244): each probe is a capture position, a
    /// box volume for parallax correction, and a PATH to its baked cubemap (a KTX2
    /// sidecar — never inlined, like `skybox_path`). Lit objects reflect the nearest
    /// applicable probe with box-projected parallax correction instead of the global
    /// skybox. The cubemaps themselves are baked by a later issue (#245).
    pub reflection_probes: crate::scene::reflection_probe::ReflectionProbeSet,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            world: World::new(),
            selected_entity_id: None,
            skybox_path: default_skybox_path(),
            ambient_color: default_ambient_color(),
            ambient_intensity: default_ambient_intensity(),
            layers: LayerRegistry::default(),
            collision_matrix: CollisionMatrix::default(),
            materials: BTreeMap::new(),
            decals: Vec::new(),
            probes: crate::scene::probe::ProbeVolume::new(),
            reflection_probes: crate::scene::reflection_probe::ReflectionProbeSet::new(),
        }
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a box-projector decal at a surface hit (the point + outward normal
    /// already produced by `Physics.Raycast`/`Physics.Shoot`). `size` is the
    /// stamp's width/height in world units; `depth` how far the box projects
    /// through the surface; `rotation_deg` spins the stamp around its axis;
    /// `color` tints the texel (alpha scales the blend); `texture` is the decal
    /// sprite (or the default checker). The registry is a bounded FIFO so spam
    /// can't grow it without limit.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_decal(
        &mut self,
        point: Vec3,
        normal: Vec3,
        size: f32,
        depth: f32,
        rotation_deg: f32,
        color: [f32; 4],
        texture: Option<String>,
    ) {
        let decal = crate::render::decals::Decal::from_hit(
            point,
            normal,
            size.max(1.0e-4),
            depth.max(1.0e-4),
            rotation_deg,
            color,
            texture,
        );
        if self.decals.len() >= crate::render::decals::MAX_DECALS {
            self.decals.remove(0);
        }
        self.decals.push(decal);
    }

    /// Drop every live decal (e.g. on level reset).
    pub fn clear_decals(&mut self) {
        self.decals.clear();
    }

    pub fn add_entity(&mut self, name: String) -> u32 {
        self.world.spawn(name)
    }

    pub fn destroy_entity(&mut self, id: u32) {
        self.world.despawn(id);
        if self.selected_entity_id == Some(id) {
            self.selected_entity_id = None;
        }
    }

    pub fn get_entity(&self, id: u32) -> Option<Ref<'_, Entity>> {
        self.world.get(id)
    }

    pub fn get_entity_mut(&mut self, id: u32) -> Option<RefMut<'_, Entity>> {
        self.world.get_mut(id)
    }

    pub fn find_entity_by_name(&self, name: &str) -> Option<u32> {
        self.world.find_by_name(name)
    }

    /// Resolve an entity's referenced material from the library, if any. Returns
    /// `None` when the entity has no `MaterialComponent` or it points at a missing
    /// library key.
    pub fn material_of(&self, entity: &Entity) -> Option<&MaterialAsset> {
        entity
            .material
            .as_ref()
            .and_then(|m| self.materials.get(&m.material))
    }

    /// Iterate entities in insertion order (immutably). Replaces the legacy
    /// `&scene.entities` iteration; each item is a borrow guard derefing to
    /// `&Entity`.
    pub fn iter(&self) -> impl Iterator<Item = Ref<'_, Entity>> {
        let world = &self.world;
        world.ids().iter().filter_map(move |&id| world.get(id))
    }

    /// Stable ids in insertion order.
    pub fn entity_ids(&self) -> Vec<u32> {
        self.world.ids().to_vec()
    }

    pub fn entity_count(&self) -> usize {
        self.world.len()
    }

    pub fn is_empty(&self) -> bool {
        self.world.is_empty()
    }

    pub fn compute_world_matrix(&self, entity_id: u32) -> Mat4 {
        if let Some(entity) = self.get_entity(entity_id) {
            let local_mat = entity.transform.to_matrix();
            if let Some(parent_id) = entity.parent_id {
                self.compute_world_matrix(parent_id) * local_mat
            } else {
                local_mat
            }
        } else {
            Mat4::IDENTITY
        }
    }

    pub fn update_entity_collider(&mut self, id: u32) {
        let parent_id = self.get_entity(id).and_then(|e| e.parent_id);
        let parent_mat = parent_id.map(|p| self.compute_world_matrix(p));
        if let Some(mut entity) = self.get_entity_mut(id) {
            entity.update_collider(parent_mat);
        }
    }

    pub fn update_all_colliders(&mut self) {
        let ids = self.world.ids().to_vec();
        for id in ids {
            self.update_entity_collider(id);
        }
    }

    pub fn set_parent(&mut self, entity_id: u32, parent_id: Option<u32>) -> Result<(), String> {
        // 1. Detect cycles
        if let Some(p_id) = parent_id {
            if entity_id == p_id {
                return Err("An entity cannot be parented to itself.".to_string());
            }

            // Traverse up from parent_id to check if entity_id is an ancestor
            let mut current = p_id;
            while let Some(ancestor_parent) = self.get_entity(current).and_then(|e| e.parent_id) {
                if ancestor_parent == entity_id {
                    return Err("Circular parenting detected (ancestor loop).".to_string());
                }
                current = ancestor_parent;
            }
        }

        // 2. Clear old parent's children list
        let old_parent_id = if let Some(entity) = self.get_entity(entity_id) {
            entity.parent_id
        } else {
            return Err("Entity not found.".to_string());
        };

        if let Some(old_p) = old_parent_id {
            if let Some(mut parent) = self.get_entity_mut(old_p) {
                parent.children.retain(|&c| c != entity_id);
            }
        }

        // 3. Set new parent and update children list
        if let Some(new_p) = parent_id {
            if let Some(mut parent) = self.get_entity_mut(new_p) {
                parent.children.push(entity_id);
            } else {
                return Err("Parent entity not found.".to_string());
            }
        }

        if let Some(mut entity) = self.get_entity_mut(entity_id) {
            entity.parent_id = parent_id;
        }

        Ok(())
    }

    /// Serialize the live World to `path`. Delegates to `crate::scene::io`,
    /// which owns the on-disk `SceneData` document.
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        crate::scene::save_to_file(self, path)
    }

    /// Load `path`, REPLACING this scene's World (single active scene).
    /// Delegates to `crate::scene::io`; meshes are rehydrated from disk there.
    pub fn load_from_file(&mut self, path: &str) -> Result<(), String> {
        crate::scene::load_from_file(self, path)
    }
}
