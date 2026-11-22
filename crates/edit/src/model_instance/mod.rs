//! Unity's model prefab: a node placing a mesh the sidecar names as another's collision mesh is not
//! spawned (spawned empty if nodes hang from it).

mod lod;

use std::borrow::Cow;

use glam::{Mat4, Vec3};

use crate::asset_instance::{collision_sub_objects, instantiate_imported};
use crate::extract_prefab;
use crate::prefab::instantiate_prefab_linked;
use crate::prefab::PrefabData;
use rusty_asset::{Imported, ModelNode};
use rusty_components::TransformComponent;
use rusty_scene::Scene;

pub fn instantiate_model(scene: &mut Scene, path: &str, asset: &Imported, position: Vec3) -> u32 {
    let mut prefab = model_prefab(path, asset);
    // Placed before the stamp, so its one collider refresh sees the parts where they land.
    let root = prefab.root;
    if let Some(entity) = prefab.entities.iter_mut().find(|e| e.id == root) {
        entity.transform.position = position;
    }
    instantiate_prefab_linked(scene, &prefab, None, path)
}

/// Keyed `<path>::#default`, never the scratch entity's own key (it would name another entity of
/// the destination scene).
pub fn model_prefab(path: &str, asset: &Imported) -> PrefabData {
    let mut scratch = Scene::new();
    let root = spawn_tree(&mut scratch, path, asset);
    let mut prefab = extract_prefab(&scratch, root).expect("the root was just spawned");
    share_default_material(&mut prefab, path);
    prefab
}

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

fn spawn_tree(scene: &mut Scene, path: &str, asset: &Imported) -> u32 {
    let root = scene.add_entity(file_stem(path));
    let nodes = asset.node_tree();
    let mut lods = lod::Lods::default();
    let mut spawned: Vec<u32> = Vec::with_capacity(nodes.len());
    let collision = collision_sub_objects(path);
    for (index, node) in nodes.iter().enumerate() {
        let parent = node.parent.and_then(|p| spawned.get(p).copied());
        let (holder, held) = holder(scene, node, parent.unwrap_or(root));
        let node = &match node.mesh.as_ref().filter(|m| collision.contains(*m)) {
            None => Cow::Borrowed(node),
            Some(_) if !nodes.iter().any(|n| n.parent == Some(index)) => {
                spawned.push(holder);
                continue;
            }
            Some(_) => Cow::Owned(ModelNode {
                mesh: None,
                ..node.clone()
            }),
        };
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
    scene.update_all_colliders();
    root
}

/// A bone the skeleton lacks leaves the node at the bone's rest pose, relative to the skinned node.
fn holder(scene: &Scene, node: &ModelNode, parent: u32) -> (u32, Mat4) {
    let Some(bone) = &node.bone else {
        return (parent, Mat4::IDENTITY);
    };
    match scene.find_bone(parent, &bone.name) {
        Some(entity) => (entity, Mat4::IDENTITY),
        None => (parent, bone.rest),
    }
}

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
    let id = match made {
        Some(Ok(id)) => id,
        _ => scene.add_entity(node.name.clone()),
    };
    let _ = scene.set_parent(id, Some(parent));
    id
}

fn place(scene: &mut Scene, id: u32, before: Mat4, node: &ModelNode) {
    if before != Mat4::IDENTITY {
        return set_matrix(scene, id, before * node.local());
    }
    if let Some(mut t) = scene.world.transform_mut(id) {
        (t.position, t.rotation, t.scale) = (node.translation, node.rotation, node.scale);
    }
}

pub(super) fn set_matrix(scene: &mut Scene, id: u32, m: Mat4) {
    if let Some(mut t) = scene.world.transform_mut(id) {
        *t = TransformComponent::from_matrix(m);
    }
}

fn file_stem(path: &str) -> String {
    let stem = std::path::Path::new(path).file_stem();
    stem.map_or_else(|| path.to_string(), |s| s.to_string_lossy().into_owned())
}

#[cfg(test)]
mod bone_tests;
#[cfg(test)]
mod gone_play_tests;
#[cfg(test)]
mod gone_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod lod_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod moved_part_tests;
#[cfg(test)]
mod removed_tests;
#[cfg(test)]
mod tests;
