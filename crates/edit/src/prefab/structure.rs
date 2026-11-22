//! What the user added under a lost part (a light, a nested instance of another model) moves to the
//! nearest surviving ancestor keeping its world place, as Unity keeps an orphaned added object.

use std::collections::{BTreeMap, BTreeSet};

use crate::prefab::instance::instance_members;
use crate::prefab::pass::Pass;
use crate::prefab::source::{node_paths, GONE};
use crate::prefab::stamp_entity;
use crate::PrefabData;
use rusty_components::{PrefabLink, TransformComponent};
use rusty_scene::prefab_link::removed::is_removed;
use rusty_scene::rehydrate::Imports;
use rusty_scene::runtime::despawn::Despawn;
use rusty_scene::{Entity, Scene};

pub(super) fn reshape(
    scene: &mut Scene,
    root: u32,
    ids: &[u32],
    source: &str,
    prefab: &PrefabData,
    pass: &mut Pass,
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
    remove_gone(scene, &gone, pass.applying);
    let mut members = instance_members(scene, root).unwrap_or_default();
    add_missing(scene, &mut members, source, &missing, &nodes, pass.imports);
    members
}

/// Only the topmost lost parts are destroyed; their subtrees go with them. The topmost are picked
/// after every move: a lost part under a surviving member under another lost part has moved out
/// with that member.
fn remove_gone(scene: &mut Scene, gone: &BTreeSet<u32>, applying: bool) {
    let bones = scene.bone_ids();
    let lost = |id: u32| gone.contains(&id) || bones.contains(&id);
    for &part in gone {
        let kept: Vec<u32> = scene
            .world
            .self_and_descendants_while(part, lost)
            .filter(|&id| !lost(id) && !scene.is_generated_hitbox(id))
            .collect();
        let Some(home) = scene.world.ancestors(part).find(|&a| !lost(a)) else {
            continue;
        };
        for id in kept {
            rehome(scene, id, home);
        }
    }
    let tops: Vec<u32> = gone
        .iter()
        .copied()
        .filter(|&id| !scene.world.ancestors(id).any(|a| gone.contains(&a)))
        .collect();
    for top in tops {
        // A document being applied has no script instances yet, so its parts go at once.
        match applying {
            true => {
                let doomed = scene.destroy_set(top);
                scene.despawn_batch(&doomed, Despawn::Applying);
            }
            false => scene.destroy(top),
        }
    }
}

fn rehome(scene: &mut Scene, id: u32, home: u32) {
    let world = scene.world.local_to_world(id);
    let local = scene.world.local_to_world(home).inverse() * world;
    if scene.set_parent(id, Some(home)).is_ok() {
        if let Some(mut t) = scene.world.transform_mut(id) {
            *t = TransformComponent::from_matrix(local);
        }
    }
}

fn add_missing(
    scene: &mut Scene,
    members: &mut Vec<u32>,
    source: &str,
    missing: &[&Entity],
    nodes: &BTreeMap<u32, String>,
    imports: &mut Imports,
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
        // An unstamped node maps to GONE: the next rebuild sets each reference from the baseline.
        let id_of = |l: u32| instance.get(&l).copied().unwrap_or(GONE);
        let mut stamped = stamp_entity(entity, id_of, &BTreeMap::new(), Some(link), imports);
        (stamped.entity.parent_id, stamped.entity.children) = (None, Vec::new());
        hung.extend(stamped.hung);
        bone_refs.extend(stamped.bone_refs);
        scene.world.insert_entity(stamped.entity);
        let _ = scene.set_parent(id, Some(parent));
        members.push(id);
    }
    if !hung.is_empty() || !bone_refs.is_empty() {
        scene.hang_on_skeletons(hung, bone_refs);
    }
}
