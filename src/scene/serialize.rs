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
//! Allowed deps: ecs, components, asset (for re-importing `"Asset"` meshes),
//! render::mesh (the `Vertex` type rehydration targets).

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::asset::{self, MeshVertex, SubMesh};
use crate::components::Entity;
use crate::render::mesh as primitives;
use crate::render::mesh::Vertex;
use crate::scene::collision_matrix::CollisionMatrix;
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
    /// Layer collision matrix. `#[serde(default)]` so pre-#91 scenes load with the
    /// all-pairs-collide default (preserving their behaviour).
    #[serde(default)]
    pub collision_matrix: CollisionMatrix,
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
        collision_matrix: scene.collision_matrix.clone(),
    }
}

/// Convert one imported `MeshVertex` (pure data) into the renderer's `Vertex`,
/// carrying the skin binding (`joint_indices`/`joint_weights`) through to the GPU
/// vertex; static meshes keep the unskinned identity binding (#79).
fn vertex_from_imported(v: &MeshVertex) -> Vertex {
    Vertex {
        position: v.position,
        normal: v.normal,
        tex_coords: v.tex_coords,
        joint_indices: v.joint_indices,
        joint_weights: v.joint_weights,
    }
}

/// Geometry + bind-pose bone palette for an imported sub-mesh, ready for the GPU
/// buffer rebuild. The palette is empty for a static (skinless) sub-mesh.
fn imported_to_render(sub: &SubMesh) -> (Vec<Vertex>, Vec<u32>, Vec<Mat4>) {
    let vertices = sub.vertices.iter().map(vertex_from_imported).collect();
    let palette = sub
        .skin
        .as_ref()
        .map(|s| s.bind_palette())
        .unwrap_or_default();
    (vertices, sub.indices.clone(), palette)
}

/// Re-import an `"Asset"` mesh's geometry + bind-pose palette from its path-based
/// `asset_ref` (`path::sub_object`). On failure (missing/renamed source — the
/// accepted path-based trade-off) the mesh rehydrates empty rather than aborting
/// the load.
fn rehydrate_asset_mesh(asset_ref: &Option<String>) -> (Vec<Vertex>, Vec<u32>, Vec<Mat4>) {
    match asset_ref {
        Some(reference) => match asset::import_sub_mesh(reference) {
            Ok(sub) => imported_to_render(&sub),
            Err(_) => (Vec::new(), Vec::new(), Vec::new()),
        },
        None => (Vec::new(), Vec::new(), Vec::new()),
    }
}

/// Tag a primitive's `(vertices, indices)` with an empty bone palette — primitives
/// are never skinned.
fn with_empty_palette(geometry: (Vec<Vertex>, Vec<u32>)) -> (Vec<Vertex>, Vec<u32>, Vec<Mat4>) {
    (geometry.0, geometry.1, Vec::new())
}

/// Rebuild each mesh's vertex/index data from its on-disk REFERENCE: a primitive
/// from `primitive_type`, or an imported sub-mesh from `asset_ref` when the type
/// is `"Asset"`. GPU buffers are never stored on disk, only rebuilt here.
fn rehydrate_meshes(data: &mut SceneData) {
    for entity in &mut data.entities {
        if let Some(mesh) = &mut entity.mesh {
            // Primitives are skinless: an empty palette leaves the GPU bones at
            // identity. Only an `"Asset"` mesh can carry a real skin (#79).
            let (vertices, indices, palette) = match mesh.primitive_type.as_str() {
                "Box" => with_empty_palette(primitives::generate_box(1.0, 1.0, 1.0)),
                "Sphere" => with_empty_palette(primitives::generate_sphere(1.0, 16, 16)),
                "Plane" => with_empty_palette(primitives::generate_plane(15.0, 15.0)),
                "Cylinder" => with_empty_palette(primitives::generate_cylinder(
                    Vec3::new(0.0, -0.5, 0.0),
                    Vec3::new(0.0, 0.5, 0.0),
                    0.5,
                    12,
                )),
                "Asset" => rehydrate_asset_mesh(&mesh.asset_ref),
                _ => (Vec::new(), Vec::new(), Vec::new()),
            };
            mesh.vertices = vertices;
            mesh.indices = indices;
            mesh.bind_palette = palette;
            mesh.is_dirty.set(true);
        }
    }
}

/// Build an `"Asset"` mesh component from a path-based reference, importing its
/// geometry up front. The reference is the only identity stored; on save the
/// vertices are dropped and re-imported from it (see `rehydrate_meshes`). Returns
/// an empty-geometry component if the import fails.
pub fn asset_mesh_component(reference: &str) -> crate::scene::MeshComponent {
    let (vertices, indices, bind_palette) = rehydrate_asset_mesh(&Some(reference.to_string()));
    crate::scene::MeshComponent {
        primitive_type: "Asset".to_string(),
        asset_ref: Some(reference.to_string()),
        vertices,
        indices,
        bind_palette,
        is_dirty: crate::scene::DirtyFlag::new(true),
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
    scene.collision_matrix = data.collision_matrix;

    scene.update_all_colliders();
}
