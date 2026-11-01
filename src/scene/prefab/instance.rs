//! src/scene/prefab/instance.rs — which entities make up a linked instance (#991).
//!
//! One rule for every prefab verb — record, revert and reimport (`link.rs`), apply to
//! source (`apply.rs`) and the inspector's prefab card: an instance is its root plus
//! every descendant that links to the root's source. Bones are **transparent**
//! (#453): a bone carries no link, but the instance's own entities may hang from one
//! (a gun on a hand), so a walk passes through a bone in either direction. Any other
//! entity not of the instance ends the walk on its side, and so does the root of
//! another instance of the same source (a linked entity with `local_id == 0`): kit
//! B placed under a part of kit A is B's, never A's (#1665).

use crate::scene::Scene;

/// The ids of the linked instance rooted at `root_id`: the root, then its members in
/// pre-order. `None` when `root_id` is not linked.
pub fn instance_members(scene: &Scene, root_id: u32) -> Option<Vec<u32>> {
    let source = scene.world.prefab_link(root_id)?.source.clone();
    let shares = |id: u32| {
        let link = scene.world.prefab_link(id);
        link.is_some_and(|l| l.source == source && (l.local_id != 0 || id == root_id))
    };
    let bones = scene.bone_ids();
    let members = scene
        .world
        .self_and_descendants_while(root_id, |id| shares(id) || bones.contains(&id))
        .filter(|&id| shares(id))
        .collect();
    Some(members)
}

/// The root of the instance `id` belongs to: its nearest ancestor (or `id` itself)
/// that is a root of the same source, reached through bones and that source's
/// parts but nothing else; failing one, the topmost such part. `None` when
/// `id` is not linked.
pub fn instance_root(scene: &Scene, id: u32) -> Option<u32> {
    let source = scene.world.prefab_link(id)?.source.clone();
    if scene.world.prefab_link(id)?.local_id == 0 {
        return Some(id);
    }
    let shares = |p: u32| {
        let link = scene.world.prefab_link(p);
        link.is_some_and(|l| l.source == source)
    };
    let bones = scene.bone_ids();
    // Up through bones and parts to the first root of this source, which ends it.
    let mut root = None;
    for p in scene.world.ancestors(id) {
        if !(shares(p) || bones.contains(&p)) {
            break;
        }
        if shares(p) {
            root = Some(p);
            if scene.world.prefab_link(p).is_some_and(|l| l.local_id == 0) {
                break;
            }
        }
    }
    Some(root.unwrap_or(id))
}

#[cfg(test)]
#[path = "instance_tests.rs"]
mod instance_tests;
