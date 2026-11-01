//! src/scene/prefab/removed.rs — a part the user takes out of a model instance
//! (#1665): Unity's *Removed GameObject* override.
//!
//! A model instance takes its model's node tree on propagation (`structure`): a
//! node no member claims is stamped in. So a part the user destroyed would come
//! back pristine on the next load, reimport or Scale Factor change unless its
//! removal is recorded. Destroying a part ([`Scene::destroy`]) records its node
//! path on the instance root's overrides
//! ([`PrefabOverrides::removed_parts`](crate::components::PrefabOverrides)), and
//! propagation stamps no node at or under a removed path ([`is_removed`]).
//! Reverting the override (the whole instance's, or that one key) brings the part
//! back.
//!
//! Moving a part out of its instance ([`Scene::reparent`]) records the same
//! removal, the part kept: Unity refuses to move a prefab instance's child out
//! without unpacking, and this is that unpack for one part. What leaves the
//! instance loses its links (plain entities now, their values kept), so it is
//! neither stamped a second time nor rebuilt from the instance's baseline.

use std::collections::BTreeSet;

use crate::scene::prefab::instance::{instance_members, instance_root};
use crate::scene::prefab::source::{is_model_source, GONE};
use crate::scene::Scene;

/// Whether node `path` is one of `removed` or under one.
pub(super) fn is_removed(removed: &BTreeSet<String>, path: &str) -> bool {
    removed.iter().any(|r| match path.strip_prefix(r.as_str()) {
        Some(rest) => rest.is_empty() || rest.starts_with('/'),
        None => false,
    })
}

/// Record `id`'s node as removed from its model instance, when `id` is a part of
/// one: the destroy about to take it is the user's (`Scene::destroy`). A part whose
/// node the model lost records nothing: propagation is what destroys it.
pub(crate) fn record_removed(scene: &mut Scene, id: u32) {
    if let Some((root, node)) = part_of(scene, id) {
        mark(scene, root, node);
    }
}

impl Scene {
    /// Re-parent `id` as the user does (`Scene.SetParent`, the inspector):
    /// [`Scene::set_parent`], and when that takes a model part out of its
    /// instance, the part is unpacked from it (module docs).
    pub fn reparent(&mut self, id: u32, parent: Option<u32>) -> Result<(), String> {
        let before = part_of(self, id)
            .and_then(|(root, node)| Some((root, node, instance_members(self, root)?)));
        self.set_parent(id, parent)?;
        if let Some((root, node, before)) = before {
            let after: BTreeSet<u32> = instance_members(self, root)
                .unwrap_or_default()
                .into_iter()
                .collect();
            let left: Vec<u32> = before.into_iter().filter(|m| !after.contains(m)).collect();
            if !left.is_empty() {
                for id in left {
                    self.world.take_prefab_link(id);
                }
                mark(self, root, node);
            }
        }
        Ok(())
    }
}

/// The instance root and node path of `id` when it is a part of a model instance:
/// linked to a model, not a root, its node still the model's.
fn part_of(scene: &Scene, id: u32) -> Option<(u32, String)> {
    let node = {
        let link = scene.world.prefab_link(id)?;
        let part = link.local_id != 0 && link.local_id != GONE;
        (part && is_model_source(&link.source)).then(|| link.node.clone())??
    };
    let root = instance_root(scene, id)?;
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
