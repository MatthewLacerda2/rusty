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
//! keeping where it is in the world. A part the user removed is not a node the
//! model gained: it is never stamped back (`removed`). A `.prefab` source stays
//! flat (#216).
//!
//! [`rekey`]: crate::scene::prefab::source::rekey

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{PrefabLink, TransformComponent};
use crate::scene::prefab::instance::instance_members;
use crate::scene::prefab::removed::is_removed;
use crate::scene::prefab::source::{node_paths, GONE};
use crate::scene::prefab::stamp_entity;
use crate::scene::runtime::despawn::Despawn;
use crate::scene::{Entity, PrefabData, Scene};

/// Give the instance at `root` (its members `ids`, already rekeyed against
/// `prefab`, the model at `source` as it imports now) the model's node tree.
/// `applying` says a document is being applied (a load, a Play snapshot): what
/// it removes has no script instance yet and goes at once; otherwise it goes the
/// way the mode allows (`Scene::destroy`). Returns the instance's members
/// afterwards: `ids` itself when the tree already matches.
pub(super) fn reshape(
    scene: &mut Scene,
    root: u32,
    ids: &[u32],
    source: &str,
    prefab: &PrefabData,
    applying: bool,
) -> Vec<u32> {
    let local = |id: &u32| scene.world.prefab_link(*id).map(|l| l.local_id);
    let gone: BTreeSet<u32> = ids
        .iter()
        .copied()
        .filter(|id| local(id) == Some(GONE))
        .collect();
    let claimed: BTreeSet<u32> = ids.iter().filter_map(local).collect();
    let removed = scene
        .world
        .prefab_link(root)
        .map(|l| l.overrides.removed_parts.clone())
        .unwrap_or_default();
    let nodes = node_paths(prefab);
    let missing: Vec<&Entity> = prefab
        .entities
        .iter()
        .filter(|e| !claimed.contains(&e.id))
        .filter(|e| nodes.get(&e.id).is_some_and(|p| !is_removed(&removed, p)))
        .collect();
    if gone.is_empty() && missing.is_empty() {
        return ids.to_vec();
    }
    remove_gone(scene, &gone, applying);
    let mut members = instance_members(scene, root).unwrap_or_default();
    add_missing(scene, &mut members, source, &missing, &nodes);
    members
}

/// Destroy every member of `gone` (its node the model lost), re-homing first
/// what else hangs under one. A node's path names its parent's, so a lost node's
/// member children are lost too: only the topmost lost part is destroyed, and its
/// subtree goes with it, the bones of a skinned part and their generated hitboxes
/// among it. Anything else reached (an entity the user added, a nested instance, a
/// surviving member the user moved under the lost part) moves out whole, as Unity
/// keeps an orphaned added object.
fn remove_gone(scene: &mut Scene, gone: &BTreeSet<u32>, applying: bool) {
    let bones = scene.bone_ids();
    let tops = gone.iter().copied().filter(|&id| {
        let mut up = scene.world.ancestors(id);
        !up.any(|a| gone.contains(&a))
    });
    for top in tops.collect::<Vec<_>>() {
        let Some(home) = scene.world.parent_id(top) else {
            continue; // the root's node path is "": never gone
        };
        // Walk the lost subtree through lost parts and bones only.
        let lost = |id: u32| gone.contains(&id) || bones.contains(&id);
        let walk = scene.world.self_and_descendants_while(top, lost);
        let kept: Vec<u32> = walk
            .filter(|&id| !lost(id) && !scene.is_generated_hitbox(id))
            .collect();
        for id in kept {
            rehome(scene, id, home);
        }
        match applying {
            true => {
                let doomed = scene.destroy_set(top);
                scene.despawn_batch(&doomed, Despawn::Applying);
            }
            false => scene.destroy(top),
        }
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

/// Stamp each of `missing` (entities of the model no member claims and the user
/// did not remove, parents first: `prefab.entities` is in subtree order) under
/// the instance entity of its parent, adding it to `members`. Its values are the
/// source's as they stand; the rebuild that follows takes its baseline and merges
/// the materials it names into the library (`Merge::KeepScene`, the model's), so
/// nothing here touches the library.
fn add_missing(
    scene: &mut Scene,
    members: &mut Vec<u32>,
    source: &str,
    missing: &[&Entity],
    nodes: &BTreeMap<u32, String>,
) {
    let mut instance: BTreeMap<u32, u32> = members
        .iter()
        .filter_map(|&id| Some((scene.world.prefab_link(id)?.local_id, id)))
        .collect();
    let (mut hung, mut bone_refs) = (Vec::new(), Vec::new());
    for entity in missing {
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
        members.push(id);
    }
    // Hanging notes every skinned entity: only when a stamp brought something.
    if !hung.is_empty() || !bone_refs.is_empty() {
        scene.hang_on_skeletons(hung, bone_refs);
    }
}
