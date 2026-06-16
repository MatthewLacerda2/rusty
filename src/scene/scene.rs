//! src/scene/scene.rs — Scene: the hecs-backed `ecs::World` plus scene-level state.
//!
//! Entity/component storage of record is `ecs::World` (one `Entity` bundle per
//! hecs entity, generational handles + stable-id/name lookup). `Scene` owns that
//! `World` together with the engine-level scene scalars (selection, skybox,
//! ambient light) and the parenting / collider / serialization helpers that
//! operate across the World — concerns that must NOT live inside `ecs::World`
//! (whose deps are restricted to hecs + components). This wrapper lives in the
//! `scene` layer, which is allowed to depend on `crate::scene` and rendering.

use glam::{Mat4, Vec3};

use crate::ecs::world::{Ref, RefMut};
use crate::ecs::World;
use crate::scene::layers::LayerRegistry;

// Re-export the component types so the many existing `scene::…Component` paths
// resolve. The structs themselves live in `crate::components`.
pub use crate::components::{
    AnimatorComponent, CameraComponent, ColliderComponent, ColliderShape, CollisionResponse,
    DirtyFlag, EmitMode, Entity, HealthComponent, LightComponent, LightType, MeshComponent,
    NavMeshAgentComponent, Particle, ParticleBlend, ParticleEmitterComponent, RigidBodyComponent,
    ScriptComponent, TextureComponent, Tonemap, TransformComponent, VisualCorrectionComponent,
};

fn default_skybox_path() -> String {
    "".to_string()
}
fn default_ambient_color() -> Vec3 {
    Vec3::new(0.03, 0.03, 0.045)
}
fn default_ambient_intensity() -> f32 {
    0.24
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
        }
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
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
