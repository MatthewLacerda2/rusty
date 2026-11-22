use std::collections::BTreeSet;

use super::instance::{instance_members_with, instance_root_with};
use super::{is_model_source, GONE};
use crate::Scene;
use rusty_components::PrefabLink;

pub fn is_removed(removed: &BTreeSet<String>, path: &str) -> bool {
    removed.iter().any(|r| match path.strip_prefix(r.as_str()) {
        Some(rest) => rest.is_empty() || rest.starts_with('/'),
        None => false,
    })
}

pub fn record_removed(scene: &mut Scene, id: u32) {
    if scene.playing {
        return;
    }
    let doomed = scene.destroy_set(id);
    let linked = |m: &&u32| scene.world.prefab_link(**m).is_some();
    if !doomed.iter().any(|m| linked(&m)) {
        // No part among them: skip the scene-wide bone scan.
        return;
    }
    let bones = scene.bone_ids();
    let parts: Vec<(u32, String)> = doomed
        .iter()
        .filter_map(|&m| part_of(scene, m, &bones))
        .filter(|(root, _)| !doomed.contains(root))
        .collect();
    for (root, node) in parts {
        mark(scene, root, node);
    }
}

impl Scene {
    pub fn reparent(&mut self, id: u32, parent: Option<u32>) -> Result<(), String> {
        let is_model_part = self
            .world
            .prefab_link(id)
            .is_some_and(|l| l.local_id != 0 && is_model_source(&l.source));
        if !is_model_part {
            return self.set_parent(id, parent);
        }
        let bones = self.bone_ids();
        let before = part_of(self, id, &bones)
            .and_then(|(root, _)| Some((root, instance_members_with(self, root, &bones)?)));
        self.set_parent(id, parent)?;
        let Some((root, before)) = before else {
            return Ok(());
        };
        let after: BTreeSet<u32> = instance_members_with(self, root, &bones)
            .unwrap_or_default()
            .into_iter()
            .collect();
        for left in before.into_iter().filter(|m| !after.contains(m)) {
            let node = self.world.take_prefab_link(left).and_then(part_node);
            if let Some(node) = node {
                mark(self, root, node);
            }
        }
        Ok(())
    }
}

fn part_node(link: PrefabLink) -> Option<String> {
    let part = link.local_id != 0 && link.local_id != GONE;
    (part && is_model_source(&link.source)).then_some(link.node)?
}

fn part_of(scene: &Scene, id: u32, bones: &BTreeSet<u32>) -> Option<(u32, String)> {
    let node = part_node(scene.world.prefab_link(id)?.clone())?;
    let root = instance_root_with(scene, id, bones)?;
    let is_root = scene
        .world
        .prefab_link(root)
        .is_some_and(|l| l.local_id == 0);
    (root != id && is_root).then_some((root, node))
}

fn mark(scene: &mut Scene, root: u32, node: String) {
    if let Some(mut link) = scene.world.prefab_link_mut(root) {
        link.overrides.removed_parts.insert(node);
    }
}
