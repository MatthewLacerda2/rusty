//! src/scene/rehydrate.rs — rebuild a mesh's runtime data from its on-disk reference.
//!
//! A saved mesh is a REFERENCE (a primitive name, or an asset's `path::sub_object`);
//! its geometry, bind palette, skeleton and clips are rebuilt here on load, prefab
//! stamp and propagation. Split out of `serialize` (#453) to keep it under the size
//! cap. GPU buffers are never stored on disk, only rebuilt here.

use glam::Mat4;

use crate::asset::{self, MeshVertex, SubMesh};
use crate::components::mesh::Vertex;
use crate::components::Entity;
use crate::scene::authoring::{primitive_geometry, Primitive};

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
        lightmap_uv: v.lightmap_uv,
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
        mesh.skeleton.bones.clear();
        mesh.is_dirty.set(true);
    }
}

/// Re-import the geometry of every unskinned mesh in `scene` instanced from the model
/// at `path`, so a changed import setting (Generate Lightmap UVs, #831) shows at once
/// rather than on the next load. Skinned meshes keep theirs: the setting never
/// touches them, and their rig is bound to live bones. Returns how many were rebuilt.
pub fn reimport_model(scene: &mut crate::scene::Scene, path: &str) -> usize {
    let prefix = format!("{path}{}", asset::REF_SEPARATOR);
    let mut rebuilt = 0;
    for id in scene.world.ids_with_mesh() {
        let Some(mut mesh) = scene.world.mesh_mut(id) else {
            continue;
        };
        let from_model = mesh
            .asset_ref
            .as_deref()
            .is_some_and(|r| r.starts_with(&prefix));
        if !from_model || mesh.skin.is_some() {
            continue;
        }
        let fresh = rehydrate_asset_mesh(&mesh.asset_ref);
        if fresh.skin.is_none() && !fresh.vertices.is_empty() {
            (mesh.vertices, mesh.indices) = (fresh.vertices, fresh.indices);
            mesh.is_dirty.set(true);
            rebuilt += 1;
        }
    }
    rebuilt
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
        skeleton: Default::default(),
        is_dirty: crate::scene::DirtyFlag::new(true),
    }
}

#[cfg(test)]
#[path = "rehydrate_tests.rs"]
mod rehydrate_tests;
