//! src/scene/serialize.rs — World <-> SceneData
//!
//! Converts between the runtime hecs-backed `Scene` and the serde `SceneData`
//! document:
//!   to_scene_data(&Scene)             -> SceneData   (read component VALUES out)
//!   apply_scene_data(&mut Scene, ..)  -> ()          (rebuild the World)
//!
//! `SceneData` is the on-disk format: entity component values + scene settings,
//! never baked GPU buffers. On load we rehydrate the non-serialized data —
//! primitive meshes from `primitive_type`, and the collider world AABBs — so the
//! file stays human-readable/diffable and small.
//!
//! Allowed deps: ecs, components.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::components::Entity;
use crate::render::mesh as primitives;
use crate::scene::layers::LayerRegistry;
use crate::scene::Scene;

fn default_skybox_path() -> String {
    String::new()
}
fn default_ambient_color() -> Vec3 {
    Vec3::new(0.03, 0.03, 0.045)
}
fn default_ambient_intensity() -> f32 {
    0.24
}

/// The on-disk scene document: entity component VALUES plus scene-level settings.
///
/// This is deliberately separate from the runtime hecs `World`: hecs does not
/// serialize a World for free, and we never want GPU buffers in the file. The
/// JSON layout matches the legacy `Scene` format so existing `.scene` files
/// round-trip unchanged.
#[derive(Serialize, Deserialize)]
pub struct SceneData {
    pub entities: Vec<Entity>,
    pub next_entity_id: u32,
    pub selected_entity_id: Option<u32>,
    #[serde(default = "default_skybox_path")]
    pub skybox_path: String,
    #[serde(default = "default_ambient_color")]
    pub ambient_color: Vec3,
    #[serde(default = "default_ambient_intensity")]
    pub ambient_intensity: f32,
    /// Project layer names. `#[serde(default)]` so pre-#90 scenes load with the
    /// stock registry ("Default" + 31 unnamed slots).
    #[serde(default)]
    pub layers: LayerRegistry,
}

/// Read the live World's component values out into a serializable document.
pub fn to_scene_data(scene: &Scene) -> SceneData {
    SceneData {
        entities: scene.world.collect_entities(),
        next_entity_id: scene.world.next_id(),
        selected_entity_id: scene.selected_entity_id,
        skybox_path: scene.skybox_path.clone(),
        ambient_color: scene.ambient_color,
        ambient_intensity: scene.ambient_intensity,
        layers: scene.layers.clone(),
    }
}

/// Rebuild a primitive mesh's vertex/index data from its `primitive_type` name.
/// GPU buffers are never stored on disk, only rebuilt here.
fn rehydrate_meshes(data: &mut SceneData) {
    for entity in &mut data.entities {
        if let Some(mesh) = &mut entity.mesh {
            let (vertices, indices) = match mesh.primitive_type.as_str() {
                "Box" => primitives::generate_box(1.0, 1.0, 1.0),
                "Sphere" => primitives::generate_sphere(1.0, 16, 16),
                "Plane" => primitives::generate_plane(15.0, 15.0),
                "Cylinder" => primitives::generate_cylinder(
                    Vec3::new(0.0, -0.5, 0.0),
                    Vec3::new(0.0, 0.5, 0.0),
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
}

/// Replace the scene's World (single active scene) with the document's contents,
/// rehydrating meshes and recomputing collider AABBs. Preserves entity order/ids.
pub fn apply_scene_data(scene: &mut Scene, mut data: SceneData) {
    rehydrate_meshes(&mut data);

    scene.world.clear();
    for entity in data.entities {
        scene.world.insert_entity(entity);
    }
    scene.world.bump_next_id(data.next_entity_id);
    scene.selected_entity_id = data.selected_entity_id;
    scene.skybox_path = data.skybox_path;
    scene.ambient_color = data.ambient_color;
    scene.ambient_intensity = data.ambient_intensity;
    data.layers.normalize();
    scene.layers = data.layers;

    scene.update_all_colliders();
}
