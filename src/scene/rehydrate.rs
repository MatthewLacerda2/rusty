//! src/scene/rehydrate.rs — rebuild a mesh's runtime data from its on-disk reference.
//!
//! A saved mesh is a REFERENCE (a primitive name, or an asset's `path::sub_object`);
//! its geometry, bind palette, skeleton and clips are rebuilt here on load, prefab
//! stamp and propagation. Split out of `serialize` (#453) to keep it under the size
//! cap. GPU buffers are never stored on disk, only rebuilt here.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};

use crate::asset::{self, MeshVertex, SubMesh};
use crate::components::mesh::Vertex;
use crate::components::{ColliderShape, Entity};
use crate::core::fs::written::Stale;
use crate::ecs::World;
use crate::scene::authoring::{primitive_geometry, Primitive};
use crate::scene::Scene;

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
        install(mesh, rebuilt);
    }
}

/// Put `rebuilt`'s geometry and rig on `mesh`, unbound from any bones: a fresh
/// mesh starts at its rest pose, and the animation system repopulates the posed
/// palette once a clip plays. `Scene::sync_skeleton` binds the bones.
fn install(mesh: &mut crate::scene::MeshComponent, rebuilt: RehydratedMesh) {
    mesh.vertices = rebuilt.vertices;
    mesh.indices = rebuilt.indices;
    mesh.bind_palette = rebuilt.bind_palette;
    mesh.skin = rebuilt.skin;
    mesh.clips = rebuilt.clips;
    mesh.pose_palette = Vec::new();
    mesh.skeleton.bones.clear();
    mesh.is_dirty.set(true);
}

/// Re-import the geometry of every unskinned mesh in `scene` instanced from the model
/// at `path`, so a changed import setting (Generate Lightmap UVs, #831) shows at once
/// rather than on the next load. Skinned meshes keep theirs: the setting never
/// touches them, and re-binding their rig would touch their bones for nothing.
/// Returns how many were rebuilt.
pub fn reimport_model(scene: &mut Scene, path: &str) -> usize {
    let prefix = format!("{path}{}", asset::REF_SEPARATOR);
    reimport_where(scene, Rig::Keep, |reference| reference.starts_with(&prefix))
}

/// Re-import every mesh in `scene` whose model file `stale` names: the engine
/// re-wrote it (#1242), and Unity updates every instance of a re-imported model.
/// A skinned one takes the new rig too, its bones re-bound to it by name (#1265). Each
/// distinct file is matched against `stale` once. Returns how many were rebuilt.
pub fn reimport_written(scene: &mut Scene, stale: &Stale) -> usize {
    if stale.is_empty() {
        return 0;
    }
    let mut named: BTreeMap<String, bool> = BTreeMap::new();
    reimport_where(scene, Rig::Rebuild, |reference| {
        let Some(file) = asset_ref_file(reference) else {
            return false;
        };
        *named
            .entry(file)
            .or_insert_with_key(|file| stale.matches(file))
    })
}

/// What a re-import does with a skinned mesh (one skinned before or after it).
#[derive(Clone, Copy, PartialEq)]
enum Rig {
    /// Leave it as it is.
    Keep,
    /// Take the new mesh and rig, and re-bind the bones to it.
    Rebuild,
}

/// Re-import every mesh whose reference `names` accepts ([`reimport_entity_mesh`]).
fn reimport_where(scene: &mut Scene, rig: Rig, mut names: impl FnMut(&str) -> bool) -> usize {
    let mut rebuilt = 0;
    for id in scene.world.ids_with_mesh() {
        let named = scene
            .world
            .mesh(id)
            .is_some_and(|mesh| mesh.asset_ref.as_deref().is_some_and(&mut names));
        if named && reimport_entity_mesh(scene, id, rig) {
            rebuilt += 1;
        }
    }
    rebuilt
}

/// Re-import one entity's mesh from its reference, and refit its mesh collider's
/// box to it. A skinned mesh (before or after) takes the new rig, its bones re-bound,
/// when `rig` says so, and kept otherwise. A model that yields nothing (gone)
/// keeps the old mesh. Returns whether the mesh was rebuilt.
fn reimport_entity_mesh(scene: &mut Scene, id: u32, rig: Rig) -> bool {
    let Some((reference, was_skinned)) = scene
        .world
        .mesh(id)
        .and_then(|m| Some((m.asset_ref.clone()?, m.skin.is_some())))
    else {
        return false;
    };
    if was_skinned && rig == Rig::Keep {
        return false;
    }
    let Ok(sub) = asset::import_sub_mesh(&reference) else {
        return false;
    };
    let skinned = was_skinned || sub.skin.is_some();
    if skinned && rig == Rig::Keep {
        return false;
    }
    let Some(bounds) = sub.bounds() else {
        return false;
    };
    // The bones are noted against the rig they were built from before it is
    // replaced, and re-bound to the new one by name.
    let hung = skinned.then(|| scene.take_skeleton(id));
    if let Some(mut mesh) = scene.world.mesh_mut(id) {
        install(&mut mesh, RehydratedMesh::from_sub_mesh(&sub));
    }
    if let Some(hung) = hung {
        scene.restore_skeleton(id, hung);
    }
    refit_mesh_collider(&mut scene.world, id, bounds);
    true
}

/// Size `id`'s mesh collider (if it has one) to `(min, max)`, as Unity's
/// `MeshCollider` follows its shared mesh. The guard refreshes the world AABB.
fn refit_mesh_collider(world: &mut World, id: u32, (min, max): (Vec3, Vec3)) {
    let Some(mut collider) = world.collider_mut(id) else {
        return;
    };
    if let ColliderShape::Mesh {
        local_min,
        local_max,
        ..
    } = &mut collider.shape
    {
        (*local_min, *local_max) = (min, max);
    }
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
#[path = "rehydrate_tests.rs"]
mod rehydrate_tests;
