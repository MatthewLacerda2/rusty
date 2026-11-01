//! src/scene/prefab/removed.rs — a part the user takes out of a model instance
//! (#1665): Unity's *Removed GameObject* override.
//!
//! A model instance takes its model's node tree on propagation (`structure`): a
//! node no member claims is stamped in. So a part the user destroyed would come
//! back pristine on the next load, reimport or Scale Factor change unless its
//! removal is recorded. Destroying a part ([`Scene::destroy`]) in edit mode
//! records its node path, and that of every part the destroy takes with it, on
//! the instance root's overrides
//! ([`PrefabOverrides::removed_parts`](crate::components::PrefabOverrides)), and
//! propagation stamps no node at or under a removed path ([`is_removed`]).
//! Reverting the override (the whole instance's, or that one key) brings the part
//! back.
//!
//! Moving a part out of its instance ([`Scene::reparent`]) records the same
//! removal for every part that leaves with it, the parts kept: Unity refuses to move a prefab instance's child out
//! without unpacking, and this is that unpack for one part. What leaves the
//! instance loses its links (plain entities now, their values kept), so it is
//! neither stamped a second time nor rebuilt from the instance's baseline.

use std::collections::BTreeSet;

use crate::components::PrefabLink;
use crate::scene::prefab::instance::{instance_members_with, instance_root_with};
use crate::scene::prefab::source::{is_model_source, GONE};
use crate::scene::Scene;

/// Whether node `path` is one of `removed` or under one.
pub(super) fn is_removed(removed: &BTreeSet<String>, path: &str) -> bool {
    removed.iter().any(|r| match path.strip_prefix(r.as_str()) {
        Some(rest) => rest.is_empty() || rest.starts_with('/'),
        None => false,
    })
}

/// Record as removed from its model instance every part the destroy of `id`
/// takes (`Scene::destroy`): `id` and each part under it, which need not be under
/// it in the model (a part moved under another inside its instance, #1665). A
/// part whose node the model lost records nothing: propagation is what destroys
/// it. Play records nothing: the scene it would be recorded on is discarded at
/// Stop, and a Play destroy stays cheap.
pub(crate) fn record_removed(scene: &mut Scene, id: u32) {
    if scene.playing {
        return;
    }
    let doomed = scene.destroy_set(id);
    let linked = |m: &&u32| scene.world.prefab_link(**m).is_some();
    if !doomed.iter().any(|m| linked(&m)) {
        return; // no part among them: skip the scene-wide bone scan
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
    /// Re-parent `id` as the user does (`Scene.SetParent`, the inspector):
    /// [`Scene::set_parent`], and when that takes model parts out of their
    /// instance (`id`, and any part under it), each is unpacked from it and
    /// recorded removed (module docs).
    pub fn reparent(&mut self, id: u32, parent: Option<u32>) -> Result<(), String> {
        let is_model_part = self
            .world
            .prefab_link(id)
            .is_some_and(|l| l.local_id != 0 && is_model_source(&l.source));
        if !is_model_part {
            return self.set_parent(id, parent);
        }
        // One scan: re-parenting changes no skeleton.
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

/// The node path of a model part's `link`: `None` for a root, or a part whose
/// node the model lost.
fn part_node(link: PrefabLink) -> Option<String> {
    let part = link.local_id != 0 && link.local_id != GONE;
    (part && is_model_source(&link.source)).then_some(link.node)?
}

/// The instance root and node path of `id` when it is a part of a model instance:
/// linked to a model, not a root, its node still the model's. `bones` is
/// [`Scene::bone_ids`].
fn part_of(scene: &Scene, id: u32, bones: &BTreeSet<u32>) -> Option<(u32, String)> {
    let node = part_node(scene.world.prefab_link(id)?.clone())?;
    let root = instance_root_with(scene, id, bones)?;
    let is_root = scene
        .world
        .prefab_link(root)
        .is_some_and(|l| l.local_id == 0);
    (root != id && is_root).then_some((root, node))
}

/// Add `node` to the removed parts on `root`'s link.
fn mark(scene: &mut Scene, root: u32, node: String) {
    if let Some(mut link) = scene.world.prefab_link_mut(root) {
        link.overrides.removed_parts.insert(node);
    }
}
