//! src/scene/model_instance/ — Instantiate a whole model as its node tree (#1083).
//!
//! What Unity's model prefab is: dropping a model into the scene spawns one parent
//! GameObject named after the file, and under it one child per node of the file
//! with that node's local transform (`asset::model_node`). A node placing a mesh
//! carries it (through the shared [`instantiate_imported`] verb, so it gets its
//! material, collider and skeleton exactly as a lone sub-object would); an empty
//! node is an empty child. So a building kit keeps its layout, a car its wheels,
//! and every pivot its place.
//!
//! A `_LOD<n>` set (#472) becomes one LODGroup where its first member node sits,
//! each level's renderer placed where its own node puts it relative to that one.

use glam::{Mat4, Vec3};

use crate::asset::lod::{lod_sets, LodSet};
use crate::asset::{ImportedAsset, ModelNode};
use crate::scene::asset_instance::instantiate_imported;
use crate::scene::lod_instance::instantiate_lod_set;
use crate::scene::Scene;

/// Spawn the model at `path` (imported as `asset`) at `position`: a root entity
/// named after the file, and its node tree under it. Returns the root's id.
pub fn instantiate_model(
    scene: &mut Scene,
    path: &str,
    asset: &ImportedAsset,
    position: Vec3,
) -> u32 {
    let root = scene.add_entity(file_stem(path));
    if let Some(mut t) = scene.world.transform_mut(root) {
        t.position = position;
    }
    let nodes = asset.node_tree();
    let worlds = crate::asset::model_node::worlds(&nodes);
    let sets = lod_sets(asset);
    // Per set: the first member node's index, and each member mesh's renderer.
    let mut groups: Vec<Option<(usize, Vec<(String, u32)>)>> = vec![None; sets.len()];
    let mut spawned: Vec<u32> = Vec::with_capacity(nodes.len());
    for (i, node) in nodes.iter().enumerate() {
        let parent = node
            .parent
            .and_then(|p| spawned.get(p).copied())
            .unwrap_or(root);
        let mesh = node.mesh.as_deref();
        let set = mesh.and_then(|m| sets.iter().position(|s| member(s, m)));
        let id = match (set, mesh) {
            (Some(k), Some(mesh)) => {
                let (first, renderers) = groups[k].get_or_insert_with(|| {
                    let group = spawn_group(scene, path, asset, &sets[k], node, parent);
                    (i, group)
                });
                let renderer = renderers.iter().find(|(m, _)| m == mesh).map(|(_, r)| *r);
                match renderer {
                    Some(r) => {
                        place(scene, r, worlds[*first].inverse() * worlds[i]);
                        r
                    }
                    None => spawn_node(scene, path, asset, node, parent),
                }
            }
            _ => spawn_node(scene, path, asset, node, parent),
        };
        spawned.push(id);
    }
    // Children spawned before they were parented: their collider AABBs are stale.
    scene.update_all_colliders();
    root
}

/// The LODGroup for `set`, spawned at `node` under `parent`: each member mesh
/// with its renderer, in level order.
fn spawn_group(
    scene: &mut Scene,
    path: &str,
    asset: &ImportedAsset,
    set: &LodSet,
    node: &ModelNode,
    parent: u32,
) -> Vec<(String, u32)> {
    let Ok(group) = instantiate_lod_set(scene, path, asset, set, None, Vec3::ZERO) else {
        return Vec::new();
    };
    set_local(scene, group, node);
    let _ = scene.set_parent(group, Some(parent));
    let renderers = scene.world.lod_group(group).map(|g| g.levels.clone());
    let renderers = renderers
        .unwrap_or_default()
        .into_iter()
        .flat_map(|l| l.renderers);
    set.levels
        .iter()
        .flatten()
        .cloned()
        .zip(renderers)
        .collect()
}

/// One node as an entity under `parent`: its mesh when it places one, else empty.
fn spawn_node(
    scene: &mut Scene,
    path: &str,
    asset: &ImportedAsset,
    node: &ModelNode,
    parent: u32,
) -> u32 {
    let name = Some(node.name.as_str());
    let made = node
        .mesh
        .as_deref()
        .map(|mesh| instantiate_imported(scene, path, asset, mesh, name, Vec3::ZERO));
    // A mesh that fails to spawn still leaves its node, so its children have a parent.
    let id = match made {
        Some(Ok(id)) => id,
        _ => scene.add_entity(node.name.clone()),
    };
    set_local(scene, id, node);
    let _ = scene.set_parent(id, Some(parent));
    id
}

fn member(set: &LodSet, mesh: &str) -> bool {
    set.levels.iter().flatten().any(|id| id == mesh)
}

/// Give `id` `node`'s local transform as authored.
fn set_local(scene: &mut Scene, id: u32, node: &ModelNode) {
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = node.translation;
        t.rotation = node.rotation;
        t.scale = node.scale;
    }
}

/// Give `id` the local transform `m`.
fn place(scene: &mut Scene, id: u32, m: Mat4) {
    let (scale, rotation, position) = m.to_scale_rotation_translation();
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = position;
        t.rotation = rotation;
        t.scale = scale;
    }
}

/// The file name of `path` without its extension: the root's name.
fn file_stem(path: &str) -> String {
    let stem = std::path::Path::new(path).file_stem();
    stem.map_or_else(|| path.to_string(), |s| s.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
