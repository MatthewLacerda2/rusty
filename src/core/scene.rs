//! src/core/scene.rs — Scene façade over the hecs-backed `ecs::World`.
//!
//! Storage now lives in `ecs::World` (one `Entity` bundle per hecs entity, with
//! generational handles and a stable-id + name lookup). `Scene` keeps the
//! engine-level scene scalars (selection, skybox, ambient light) and the
//! parenting / collider / serialization helpers that operate across the World.
//!
//! The component structs themselves were moved to `crate::components`; they are
//! re-exported here so the many existing consumers
//! (`use crate::core::scene::{…Component}`) keep compiling unchanged.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::ecs::world::{Ref, RefMut};
use crate::ecs::World;

// Re-export the migrated component types so legacy `core::scene::…` paths resolve.
pub use crate::components::{
    AnimatorComponent, CameraComponent, ColliderComponent, ColliderShape, DirtyFlag, Entity,
    HealthComponent, LightComponent, LightType, MeshComponent, NavMeshAgentComponent,
    RigidBodyComponent, ScriptComponent, TextureComponent, TransformComponent,
    VisualCorrectionComponent,
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
}

/// On-disk document. Matches the legacy `Scene` JSON layout exactly so existing
/// `.scene` files round-trip unchanged.
#[derive(Serialize, Deserialize)]
struct SceneData {
    entities: Vec<Entity>,
    next_entity_id: u32,
    selected_entity_id: Option<u32>,
    #[serde(default = "default_skybox_path")]
    skybox_path: String,
    #[serde(default = "default_ambient_color")]
    ambient_color: Vec3,
    #[serde(default = "default_ambient_intensity")]
    ambient_intensity: f32,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            world: World::new(),
            selected_entity_id: None,
            skybox_path: default_skybox_path(),
            ambient_color: default_ambient_color(),
            ambient_intensity: default_ambient_intensity(),
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

    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let data = SceneData {
            entities: self.world.collect_entities(),
            next_entity_id: self.world.next_id(),
            selected_entity_id: self.selected_entity_id,
            skybox_path: self.skybox_path.clone(),
            ambient_color: self.ambient_color,
            ambient_intensity: self.ambient_intensity,
        };
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Failed to serialize scene: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to write scene file: {}", e))?;
        Ok(())
    }

    pub fn load_from_file(&mut self, path: &str) -> Result<(), String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read scene file: {}", e))?;
        let mut data: SceneData = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize scene: {}", e))?;

        // Regenerate primitive meshes based on primitive_type
        for entity in &mut data.entities {
            if let Some(mesh) = &mut entity.mesh {
                let p_type = mesh.primitive_type.as_str();
                let (vertices, indices) = match p_type {
                    "Box" => crate::render::mesh::generate_box(1.0, 1.0, 1.0),
                    "Sphere" => crate::render::mesh::generate_sphere(1.0, 16, 16),
                    "Plane" => crate::render::mesh::generate_plane(15.0, 15.0),
                    "Cylinder" => crate::render::mesh::generate_cylinder(
                        glam::Vec3::new(0.0, -0.5, 0.0),
                        glam::Vec3::new(0.0, 0.5, 0.0),
                        0.5,
                        12,
                    ),
                    _ => (Vec::new(), Vec::new()),
                };
                mesh.vertices = vertices;
                mesh.indices = indices;
                mesh.is_dirty.set(true);
            }
        }

        // Rebuild the World from the loaded entities, preserving order + ids.
        self.world.clear();
        for entity in data.entities {
            self.world.insert_entity(entity);
        }
        self.world.bump_next_id(data.next_entity_id);
        self.selected_entity_id = data.selected_entity_id;
        self.skybox_path = data.skybox_path;
        self.ambient_color = data.ambient_color;
        self.ambient_intensity = data.ambient_intensity;

        self.update_all_colliders();
        Ok(())
    }
}
