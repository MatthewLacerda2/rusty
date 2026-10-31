//! src/scene/model_instance/ — Instantiate a whole model as its node tree (#1083).
//!
//! What Unity's model prefab is: dropping a model into the scene spawns one parent
//! GameObject named after the file, and under it one child per node of the file
//! with that node's local transform (`asset::model_node`). A node placing a mesh
//! carries it (through the shared [`instantiate_imported`] verb, so it gets its
//! material, collider and skeleton exactly as a lone sub-object would); an empty
//! node is an empty child. So a building kit keeps its layout, a car its wheels,
//! and every pivot its place. A node a joint holds (a gun in `Hand`) hangs from
//! the skinned node's bone entity (#453), so it follows the animation.
//!
//! The `_LOD<n>` nodes under one parent sharing a base (and Blender's `.001`
//! duplicate suffix) become one LODGroup where the first of them sits (`lod`).
//! A later Scale Factor change re-imports the meshes but does not move the parts
//! yet: an instance keeps no link to its model, so it cannot be told apart from
//! entities that merely share its names (#1637, models as prefab sources).

mod lod;

use glam::{Mat4, Vec3};

use crate::asset::{ImportedAsset, ModelNode};
use crate::components::TransformComponent;
use crate::scene::asset_instance::instantiate_imported;
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
    let mut lods = lod::Lods::default();
    let mut spawned: Vec<u32> = Vec::with_capacity(nodes.len());
    for node in nodes.iter() {
        let parent = node.parent.and_then(|p| spawned.get(p).copied());
        let (holder, held) = holder(scene, node, parent.unwrap_or(root));
        let id = match lods.group_for(scene, node, holder, held) {
            Some((group, at)) => {
                let id = spawn_node(scene, path, asset, node, group);
                set_matrix(scene, id, at);
                lods.add(group, node, id);
                id
            }
            None => {
                let id = spawn_node(scene, path, asset, node, holder);
                place(scene, id, held, node);
                id
            }
        };
        spawned.push(id);
    }
    lods.finish(scene, asset);
    // Children spawned before they were parented: their collider AABBs are stale.
    scene.update_all_colliders();
    root
}

/// Where `node` hangs under its parent node's entity `parent`: the bone it names,
/// or `parent` itself, with what sits between them and the node's own local
/// transform (a bone the skeleton lacks leaves the node at the bone's rest pose,
/// relative to the skinned node).
fn holder(scene: &Scene, node: &ModelNode, parent: u32) -> (u32, Mat4) {
    let Some(bone) = &node.bone else {
        return (parent, Mat4::IDENTITY);
    };
    match scene.find_bone(parent, &bone.name) {
        Some(entity) => (entity, Mat4::IDENTITY),
        None => (parent, bone.rest),
    }
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
    let _ = scene.set_parent(id, Some(parent));
    id
}

/// Give `id` the local transform `before · node.local()`: `node`'s as authored
/// when nothing sits before it, so a node's own TRS is never re-split.
fn place(scene: &mut Scene, id: u32, before: Mat4, node: &ModelNode) {
    if before != Mat4::IDENTITY {
        return set_matrix(scene, id, before * node.local());
    }
    if let Some(mut t) = scene.world.transform_mut(id) {
        (t.position, t.rotation, t.scale) = (node.translation, node.rotation, node.scale);
    }
}

/// Give `id` the local transform `m`, a product of the file's node matrices.
pub(super) fn set_matrix(scene: &mut Scene, id: u32, m: Mat4) {
    if let Some(mut t) = scene.world.transform_mut(id) {
        *t = TransformComponent::from_matrix(m);
    }
}

/// The file name of `path` without its extension: the root's name.
fn file_stem(path: &str) -> String {
    let stem = std::path::Path::new(path).file_stem();
    stem.map_or_else(|| path.to_string(), |s| s.to_string_lossy().into_owned())
}

#[cfg(test)]
mod bone_tests;
#[cfg(test)]
mod lod_tests;
#[cfg(test)]
mod tests;
