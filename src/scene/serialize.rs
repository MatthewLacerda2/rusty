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

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::asset::{self, MeshVertex, SubMesh};
use crate::components::{Entity, MaterialAsset};
use crate::render::mesh::Vertex;
use crate::scene::authoring::{primitive_geometry, Primitive};
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
    /// The material library: reusable materials keyed by name (#201). Entities
    /// reference one by name. `#[serde(default)]` so pre-#201 scenes (which stored
    /// the material inline per entity) load with an empty library; their inline
    /// materials are migrated in [`apply_scene_data`].
    #[serde(default)]
    pub materials: BTreeMap<String, MaterialAsset>,
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
        materials: scene.materials.clone(),
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
        tangent: v.tangent,
    }
}

/// The non-serialized data a mesh component is rebuilt with on load: GPU-ready
/// geometry plus the imported rig (bind palette, skeleton, clips). A primitive or a
/// failed import yields the empty default — no skin, no clips, GPU bones at identity.
#[derive(Default)]
struct RehydratedMesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    bind_palette: Vec<Mat4>,
    skin: Option<crate::asset::SkinData>,
    clips: Vec<crate::asset::AnimationClip>,
}

impl RehydratedMesh {
    /// A primitive's geometry with no rig — primitives are never skinned/animated.
    fn primitive(geometry: (Vec<Vertex>, Vec<u32>)) -> Self {
        Self {
            vertices: geometry.0,
            indices: geometry.1,
            ..Self::default()
        }
    }

    /// Pull geometry + the full rig (bind palette, skeleton, clips) out of an
    /// imported sub-mesh.
    fn from_sub_mesh(sub: &SubMesh) -> Self {
        let bind_palette = sub
            .skin
            .as_ref()
            .map(|s| s.bind_palette())
            .unwrap_or_default();
        Self {
            vertices: sub.vertices.iter().map(vertex_from_imported).collect(),
            indices: sub.indices.clone(),
            bind_palette,
            skin: sub.skin.clone(),
            clips: sub.clips.clone(),
        }
    }
}

/// Re-import an `"Asset"` mesh's geometry + rig from its path-based `asset_ref`
/// (`path::sub_object`). On failure (missing/renamed source — the accepted
/// path-based trade-off) the mesh rehydrates empty rather than aborting the load.
fn rehydrate_asset_mesh(asset_ref: &Option<String>) -> RehydratedMesh {
    match asset_ref {
        Some(reference) => match asset::import_sub_mesh(reference) {
            Ok(sub) => RehydratedMesh::from_sub_mesh(&sub),
            Err(_) => RehydratedMesh::default(),
        },
        None => RehydratedMesh::default(),
    }
}

/// Rebuild one mesh's vertex/index data + rig from its on-disk REFERENCE: a
/// primitive from `primitive_type`, or an imported sub-mesh from `asset_ref` when
/// the type is `"Asset"`. GPU buffers are never stored on disk, only rebuilt here.
fn rehydrate_one(primitive_type: &str, asset_ref: &Option<String>) -> RehydratedMesh {
    if primitive_type == "Asset" {
        return rehydrate_asset_mesh(asset_ref);
    }
    // Primitive geometry is owned by `scene::authoring` so a created primitive and
    // a loaded one are byte-identical.
    match Primitive::parse(primitive_type).and_then(primitive_geometry) {
        Some(geometry) => RehydratedMesh::primitive(geometry),
        None => RehydratedMesh::default(),
    }
}

/// Rebuild one entity's mesh (if any) from its on-disk reference. Shared by scene
/// load and prefab instantiate (#215) so both rehydrate identically; GPU buffers
/// are never stored on disk, only rebuilt here.
pub fn rehydrate_entity_mesh(entity: &mut Entity) {
    if let Some(mesh) = &mut entity.mesh {
        let rebuilt = rehydrate_one(&mesh.primitive_type, &mesh.asset_ref);
        mesh.vertices = rebuilt.vertices;
        mesh.indices = rebuilt.indices;
        mesh.bind_palette = rebuilt.bind_palette;
        mesh.skin = rebuilt.skin;
        mesh.clips = rebuilt.clips;
        // A freshly rehydrated mesh starts at its rest pose; the animation
        // system repopulates the posed palette once a clip plays.
        mesh.pose_palette = Vec::new();
        mesh.is_dirty.set(true);
    }
}

/// Rebuild every mesh in the document from its reference (see [`rehydrate_one`]).
fn rehydrate_meshes(data: &mut SceneData) {
    for entity in &mut data.entities {
        rehydrate_entity_mesh(entity);
    }
}

/// Build an `"Asset"` mesh component from a path-based reference, importing its
/// geometry up front. The reference is the only identity stored; on save the
/// vertices are dropped and re-imported from it (see `rehydrate_meshes`). Returns
/// an empty-geometry component if the import fails.
pub fn asset_mesh_component(reference: &str) -> crate::scene::MeshComponent {
    let rebuilt = rehydrate_asset_mesh(&Some(reference.to_string()));
    crate::scene::MeshComponent {
        primitive_type: "Asset".to_string(),
        asset_ref: Some(reference.to_string()),
        vertices: rebuilt.vertices,
        indices: rebuilt.indices,
        bind_palette: rebuilt.bind_palette,
        skin: rebuilt.skin,
        clips: rebuilt.clips,
        pose_palette: Vec::new(),
        is_dirty: crate::scene::DirtyFlag::new(true),
    }
}

/// Replace the scene's World (single active scene) with the document's contents,
/// rehydrating meshes and recomputing collider AABBs. Preserves entity order/ids.
pub fn apply_scene_data(scene: &mut Scene, mut data: SceneData) {
    rehydrate_meshes(&mut data);

    // Start from the document's library (new-format scenes), then fold in any legacy
    // inline materials migration carriers lifted out of pre-#201 entities by
    // `From<EntityRepr>`. Done before spawning so each entity's reference resolves.
    scene.materials = data.materials;
    for entity in &mut data.entities {
        if let Some(pending) = entity.pending_material.take() {
            if let Some(reference) = &entity.material {
                scene.materials.insert(reference.material.clone(), pending);
            }
        }
    }

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
