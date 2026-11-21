use std::collections::BTreeSet;

use crate::Scene;

pub fn instance_members(scene: &Scene, root_id: u32) -> Option<Vec<u32>> {
    instance_members_with(scene, root_id, &scene.bone_ids())
}

pub(super) fn instance_members_with(
    scene: &Scene,
    root_id: u32,
    bones: &BTreeSet<u32>,
) -> Option<Vec<u32>> {
    let source = scene.world.prefab_link(root_id)?.source.clone();
    let shares = |id: u32| {
        let link = scene.world.prefab_link(id);
        link.is_some_and(|l| l.source == source && (l.local_id != 0 || id == root_id))
    };
    let members = scene
        .world
        .self_and_descendants_while(root_id, |id| shares(id) || bones.contains(&id))
        .filter(|&id| shares(id))
        .collect();
    Some(members)
}

pub fn instance_root(scene: &Scene, id: u32) -> Option<u32> {
    instance_root_with(scene, id, &scene.bone_ids())
}

pub(super) fn instance_root_with(scene: &Scene, id: u32, bones: &BTreeSet<u32>) -> Option<u32> {
    let source = scene.world.prefab_link(id)?.source.clone();
    if scene.world.prefab_link(id)?.local_id == 0 {
        return Some(id);
    }
    let shares = |p: u32| {
        let link = scene.world.prefab_link(p);
        link.is_some_and(|l| l.source == source)
    };
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
