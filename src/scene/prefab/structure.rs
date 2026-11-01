//! src/scene/prefab/structure.rs — a model instance takes its model's new shape
//! (#1665, Unity's model prefab).
//!
//! Propagation matches instance entities to source entities ([`rekey`] gives each
//! its local id by node path), and for a model source this pass then makes the
//! instance's node tree the model's: a node the model gained is stamped in under
//! its parent node's entity, and a part whose node the model lost is destroyed.
//! What the user added under a lost part (an entity of no link to this instance:
//! a light, a nested instance of another model) is not lost with it: as Unity
//! keeps an orphaned added object, it moves to the nearest surviving ancestor,
//! keeping where it is in the world. A `.prefab` source stays flat (#216).
//!
//! [`rekey`]: crate::scene::prefab::source::rekey

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{PrefabLink, TransformComponent};
use crate::scene::prefab::instance::instance_members;
use crate::scene::prefab::source::{node_paths, GONE};
use crate::scene::prefab::stamp_entity;
use crate::scene::{PrefabData, Scene};

/// Give the instance at `root` (its members `ids`, already rekeyed against
/// `prefab`, the model at `source` as it imports now) the model's node tree.
/// Returns the instance's members afterwards.
pub(super) fn reshape(
    scene: &mut Scene,
    root: u32,
    ids: &[u32],
    source: &str,
    prefab: &PrefabData,
) -> Vec<u32> {
    remove_gone(scene, ids);
    add_missing(scene, root, source, prefab);
    instance_members(scene, root).unwrap_or_default()
}

/// Destroy every member whose node the model lost, re-homing first what the user
/// added under one. A node's path names its parent's, so a lost node's member
/// children are lost too: only the topmost lost part is destroyed, and its subtree
/// goes with it, the bones of a skinned part among it.
fn remove_gone(scene: &mut Scene, ids: &[u32]) {
    let members: BTreeSet<u32> = ids.iter().copied().collect();
    let gone: BTreeSet<u32> = ids
        .iter()
        .copied()
        .filter(|&id| {
            scene
                .world
                .prefab_link(id)
                .is_some_and(|l| l.local_id == GONE)
        })
        .collect();
    let bones = scene.bone_ids();
    let tops = gone.iter().copied().filter(|&id| {
        let mut up = scene.world.ancestors(id);
        !up.any(|a| gone.contains(&a))
    });
    for top in tops.collect::<Vec<_>>() {
        let Some(home) = scene.world.parent_id(top) else {
            continue; // the root's node path is "": never gone
        };
        // Walk the lost subtree through lost parts and bones only; anything else
        // reached is the user's (or a nested instance) and moves out whole.
        let lost = |id: u32| gone.contains(&id) || bones.contains(&id);
        let walk = scene.world.self_and_descendants_while(top, lost);
        let added: Vec<u32> = walk
            .filter(|&id| !lost(id) && !members.contains(&id))
            .collect();
        for id in added {
            rehome(scene, id, home);
        }
        scene.destroy(top);
    }
}

/// Parent `id` under `home`, keeping its world transform.
fn rehome(scene: &mut Scene, id: u32, home: u32) {
    let world = scene.world.local_to_world(id);
    let local = scene.world.local_to_world(home).inverse() * world;
    if scene.set_parent(id, Some(home)).is_ok() {
        if let Some(mut t) = scene.world.transform_mut(id) {
            *t = TransformComponent::from_matrix(local);
        }
    }
}

/// Stamp every entity of `prefab` no member claims under the instance entity of
/// its parent, parents first (`prefab.entities` is in subtree order). Its values
/// are the source's as they stand; the rebuild that follows takes its baseline
/// and merges the materials it names into the library (`Merge::KeepScene`, the
/// model's), so nothing here touches the library.
fn add_missing(scene: &mut Scene, root: u32, source: &str, prefab: &PrefabData) {
    let members = instance_members(scene, root).unwrap_or_default();
    let mut instance: BTreeMap<u32, u32> = members
        .iter()
        .filter_map(|&id| Some((scene.world.prefab_link(id)?.local_id, id)))
        .collect();
    let nodes = node_paths(prefab);
    let (mut hung, mut bone_refs) = (Vec::new(), Vec::new());
    for entity in &prefab.entities {
        if instance.contains_key(&entity.id) {
            continue;
        }
        let Some(parent) = entity.parent_id.and_then(|p| instance.get(&p).copied()) else {
            continue;
        };
        let id = scene.world.next_id();
        instance.insert(entity.id, id);
        let link = PrefabLink {
            source: source.to_string(),
            local_id: entity.id,
            node: nodes.get(&entity.id).cloned(),
            ..Default::default()
        };
        // A reference to a node not stamped yet has no id: the rebuild that
        // follows sets every reference from the baseline anyway.
        let id_of = |l: u32| instance.get(&l).copied().unwrap_or(GONE);
        let mut stamped = stamp_entity(entity, id_of, &BTreeMap::new(), Some(link));
        // Linked through `set_parent`, so the parent's children list has it.
        (stamped.entity.parent_id, stamped.entity.children) = (None, Vec::new());
        hung.extend(stamped.hung);
        bone_refs.extend(stamped.bone_refs);
        scene.world.insert_entity(stamped.entity);
        let _ = scene.set_parent(id, Some(parent));
    }
    scene.hang_on_skeletons(hung, bone_refs);
}
