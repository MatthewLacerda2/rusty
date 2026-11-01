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
//!
//! The spawn is a **linked prefab instance of the model** (#1637, Unity's model
//! prefab): [`model_prefab`] builds the tree in a scratch scene and extracts it as
//! a [`PrefabData`], and [`instantiate_model`] stamps that with a link to the file
//! on every entity, keyed by node path (`scene::prefab::source`). So a Scale
//! Factor change or a re-imported file reaches every instance (at once in the open
//! scene, on load in the others), a part moved by hand keeps its override, and a
//! node added to or removed from the file is added to or removed from every
//! instance (`scene::prefab::structure`, #1665).

mod lod;

use glam::{Mat4, Vec3};

use crate::asset::{Imported, ModelNode};
use crate::components::TransformComponent;
use crate::scene::asset_instance::instantiate_imported;
use crate::scene::prefab::instantiate_prefab_linked;
use crate::scene::prefab::PrefabData;
use crate::scene::{extract_prefab, Scene};

/// Spawn the model at `path` (imported as `asset`) at `position` as a linked
/// instance of it: a root entity named after the file, and its node tree under
/// it. Returns the root's id.
pub fn instantiate_model(scene: &mut Scene, path: &str, asset: &Imported, position: Vec3) -> u32 {
    let mut prefab = model_prefab(path, asset);
    // Placed before the stamp, so its one collider refresh sees the parts where they
    // land; a root's placement is its own, never an override.
    let root = prefab.root;
    if let Some(entity) = prefab.entities.iter_mut().find(|e| e.id == root) {
        entity.transform.position = position;
    }
    instantiate_prefab_linked(scene, &prefab, None, path)
}

/// The model at `path` (imported as `asset`) as a prefab: its node tree spawned
/// into a scratch scene and extracted. A node with no material of its own draws
/// with one engine default per model, keyed `<path>::#default`, never the scratch
/// entity's own key, which would name some other entity of the scene it is
/// stamped into.
pub fn model_prefab(path: &str, asset: &Imported) -> PrefabData {
    let mut scratch = Scene::new();
    let root = spawn_tree(&mut scratch, path, asset);
    let mut prefab = extract_prefab(&scratch, root).expect("the root was just spawned");
    share_default_material(&mut prefab, path);
    prefab
}

/// Rename every per-entity default material of `prefab` to `<path>::#default`.
fn share_default_material(prefab: &mut PrefabData, path: &str) {
    let is_own = |key: &str| {
        let id = key
            .strip_prefix("entity_")
            .and_then(|k| k.strip_suffix("_material"));
        id.is_some_and(|id| id.parse::<u32>().is_ok())
    };
    let shared = format!("{path}::#default");
    let owned: Vec<String> = prefab
        .materials
        .keys()
        .filter(|k| is_own(k))
        .cloned()
        .collect();
    for key in owned {
        if let Some(asset) = prefab.materials.remove(&key) {
            prefab.materials.entry(shared.clone()).or_insert(asset);
        }
    }
    let slots = prefab.entities.iter_mut().flat_map(|e| &mut e.material);
    for key in slots.flat_map(|m| &mut m.materials) {
        if is_own(key) {
            key.clone_from(&shared);
        }
    }
}

/// Spawn the root and the node tree under it into `scene`. Returns the root.
fn spawn_tree(scene: &mut Scene, path: &str, asset: &Imported) -> u32 {
    let root = scene.add_entity(file_stem(path));
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
    asset: &Imported,
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
mod gone_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod lod_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod tests;
