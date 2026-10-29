//! src/scene/rehydrate/mod.rs — rebuild a mesh's runtime data from its on-disk reference.
//!
//! A saved mesh is a REFERENCE (a primitive name, or an asset's `path::sub_object`);
//! its geometry, bind palette, skeleton and clips are rebuilt here on load, prefab
//! stamp and propagation. Split out of `serialize` (#453) to keep it under the size
//! cap. GPU buffers are never stored on disk, only rebuilt here.

pub mod model_writes;
mod reimport;

pub use reimport::{reimport_model, reimport_model_ids, reimport_written, Rig};

use glam::{Mat4, Vec3};

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
    ranges: Vec<std::ops::Range<u32>>,
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
            ranges: sub.ranges.iter().map(|r| r.indices.clone()).collect(),
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
            Err(e) => {
                warn_once(reference, &e);
                RehydratedMesh::default()
            }
        },
        None => RehydratedMesh::default(),
    }
}

thread_local! {
    /// The references a failed rehydrate was already warned of: a scene reloads
    /// often, and one warning per reference is enough (#1295).
    static WARNED: std::cell::RefCell<std::collections::BTreeSet<String>> =
        const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
}

/// Say why `reference`'s mesh comes back empty, once per reference, rather than
/// leaving an invisible entity with no explanation.
fn warn_once(reference: &str, error: &asset::ImportError) {
    let first = WARNED.with(|warned| warned.borrow_mut().insert(reference.to_string()));
    if first {
        log::warn!("[Scene] mesh '{reference}' loads empty: {error}");
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
/// A mesh collider's saved box is refit to the rebuilt mesh, so an import setting
/// changed since the save (the Scale Factor, #1398) reaches it.
pub fn rehydrate_entity_mesh(entity: &mut Entity) {
    if let Some(mesh) = &mut entity.mesh {
        let rebuilt = rehydrate_one(&mesh.primitive_type, &mesh.asset_ref);
        install(mesh, rebuilt);
        let positions = mesh.vertices.iter().map(|v| Vec3::from_array(v.position));
        let bounds = positions.fold(None, |b: Option<(Vec3, Vec3)>, p| {
            Some(b.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))))
        });
        if let (Some(bounds), Some(collider)) = (bounds, &mut entity.collider) {
            reimport::refit(collider, bounds);
        }
    }
}

/// Put `rebuilt`'s geometry and rig on `mesh`, unbound from any bones: a fresh
/// mesh starts at its rest pose, and the animation system repopulates the posed
/// palette once a clip plays. `Scene::sync_skeleton` binds the bones.
fn install(mesh: &mut crate::scene::MeshComponent, rebuilt: RehydratedMesh) {
    mesh.vertices = rebuilt.vertices;
    mesh.indices = rebuilt.indices;
    mesh.ranges = rebuilt.ranges;
    mesh.bind_palette = rebuilt.bind_palette;
    mesh.skin = rebuilt.skin;
    mesh.clips = rebuilt.clips;
    mesh.pose_palette = Vec::new();
    mesh.skeleton.bones.clear();
    mesh.revision = crate::components::MeshRevision::next();
}

/// Build an `"Asset"` mesh component from a path-based reference, importing its
/// geometry up front. The reference is the only identity stored; on save the
/// vertices are dropped and re-imported from it (see `rehydrate_meshes`). Returns
/// an empty-geometry component if the import fails.
pub fn asset_mesh_component(reference: &str) -> crate::scene::MeshComponent {
    let rebuilt = rehydrate_asset_mesh(&Some(reference.to_string()));
    let mut mesh = crate::scene::MeshComponent {
        asset_ref: Some(reference.to_string()),
        ..crate::scene::MeshComponent::new("Asset", Vec::new(), Vec::new())
    };
    install(&mut mesh, rebuilt);
    mesh
}

/// The file a mesh reference rehydrates from (`models/a.fbx::Body` →
/// `models/a.fbx`): what a renderer cache keyed by reference matches against the
/// write log (#1219). `None` for a string that is not a reference (a primitive).
pub fn asset_ref_file(reference: &str) -> Option<String> {
    asset::AssetRef::parse(reference).ok().map(|r| r.path)
}

#[cfg(test)]
mod tests;
