//! Re-binding a live skeleton to a re-imported rig (#1252, #1265).
//!
//! When a skinned model is re-written, every entity built from it takes the new
//! mesh and rig, and its bones are re-bound to the new rig **in place, by name**,
//! as Unity keeps a model's bone Transforms on a re-import: a bone the new rig
//! still names keeps its entity (id, `active`, `layer`, components, children,
//! the selection), moved to its new parent bone and to the new rest pose unless
//! it was off the old one (an override). A bone new to the rig is spawned at
//! rest. A bone the rig dropped is despawned; what hung from it hangs from its
//! nearest surviving ancestor by the same rule as a load (`hang`), and an id
//! leaf naming it is re-bound the same way (`bone_refs`). A model re-written as
//! static drops every bone, with a warning for what was authored on them.

use std::collections::{BTreeMap, BTreeSet};

use super::bone_refs::BoneRef;
use super::hang::Hung;
use super::persist::{bone_chain, carries_components, OVERRIDE_EPS};
use super::sync::rest_local;
use crate::components::TransformComponent;
use crate::scene::Scene;

/// What [`Scene::take_skeleton`] notes of `owner`'s bones against the old rig,
/// for [`Scene::restore_skeleton`] to re-bind them to the new one.
#[derive(Debug, Default)]
pub struct Parked {
    /// Each live bone, in slot order: its id and its bone chain (its name first).
    bones: Vec<(u32, Vec<String>)>,
    /// The bones off the old rig's rest pose, by id: a bone the new rig keeps
    /// keeps its pose. Keyed by entity, not name, so two joints of one name each
    /// keep their own (#1277); only a save keys them by name.
    overrides: BTreeMap<u32, TransformComponent>,
}

impl Scene {
    /// Note `owner`'s bones against the rig they were built from, before it is
    /// replaced: each bone's name and chain, and which were moved off its rest
    /// pose. Nothing is despawned. Empty for an entity with no skin. A bone moved
    /// out from under `owner` is no longer a bone, and is left where it is.
    pub(crate) fn take_skeleton(&self, owner: u32) -> Parked {
        let Some(skin) = self.world.mesh(owner).and_then(|m| m.skin.clone()) else {
            return Parked::default();
        };
        let mut parked = Parked::default();
        for (slot, bone) in self.live_slots(owner) {
            let Some(transform) = self.world.transform(bone).map(|t| t.clone()) else {
                continue;
            };
            if !transform.approx_eq(&rest_local(&skin, slot), OVERRIDE_EPS) {
                parked.overrides.insert(bone, transform);
            }
            parked.bones.push((bone, bone_chain(&skin, slot)));
        }
        parked
    }

    /// Re-bind `parked` to `owner`'s (new) rig: keep each bone the rig still
    /// names, spawn the new ones, and despawn the rest, hanging what hung from
    /// them, and re-binding the leaves naming them, by bone name.
    pub(crate) fn restore_skeleton(&mut self, owner: u32, parked: Parked) {
        let Parked {
            bones: old,
            mut overrides,
        } = parked;
        let skin = self.world.mesh(owner).and_then(|m| m.skin.clone());
        // Name → the old bones of that name, last slot first, so `pop` takes the first.
        let mut pool: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
        for (id, chain) in old.iter().rev() {
            pool.entry(chain[0].as_str()).or_default().push(*id);
        }
        let bones = skin.map_or_else(Vec::new, |skin| {
            self.walk_slots(owner, &skin, |scene, slot, parent| {
                let name = skin.name(slot);
                let rest = rest_local(&skin, slot);
                let Some(bone) = pool.get_mut(name.as_str()).and_then(Vec::pop) else {
                    return scene.spawn_bone(parent, name, rest);
                };
                if scene.world.parent_id(bone) != Some(parent) {
                    let _ = scene.set_parent(bone, Some(parent));
                }
                let pose = overrides.remove(&bone).unwrap_or(rest);
                if let Some(mut t) = scene.world.transform_mut(bone) {
                    *t = pose;
                }
                bone
            })
        });
        let left: BTreeSet<u32> = pool.into_values().flatten().collect();
        if let Some(mut mesh) = self.world.mesh_mut(owner) {
            mesh.skeleton.bones = bones;
        }
        let removed: BTreeMap<u32, Vec<String>> = old
            .into_iter()
            .filter(|(id, _)| left.contains(id))
            .collect();
        self.drop_bones(owner, &removed, overrides);
    }

    /// Despawn `removed`, `owner`'s bones the new rig no longer has: what hung
    /// from them hangs on by bone name, the leaves naming them re-bind the same
    /// way, and what was authored on them (`overrides`, the removed bones' own) is
    /// warned about. Nothing removed (a vertex-only re-import) costs nothing.
    fn drop_bones(
        &mut self,
        owner: u32,
        removed: &BTreeMap<u32, Vec<String>>,
        overrides: BTreeMap<u32, TransformComponent>,
    ) {
        if removed.is_empty() {
            return;
        }
        let mut hung: Vec<Hung> = Vec::new();
        for (&bone, chain) in removed {
            if overrides.contains_key(&bone) {
                log::warn!(
                    "[Asset] entity {owner}: the skeleton has no bone '{}' any more; \
                     its transform override was dropped.",
                    chain[0]
                );
            }
            self.warn_dropped_bone(owner, bone, &chain[0]);
            for child in self.world.children(bone) {
                if !removed.contains_key(&child) && !self.is_bone(child) {
                    hung.push((child, chain.clone()));
                }
            }
        }
        for (child, _) in &hung {
            let _ = self.set_parent(*child, Some(owner));
        }
        let refs: Vec<BoneRef> = self.park_bone_refs(owner, removed);
        let doomed = self.subtrees(removed.keys().copied().collect());
        self.despawn_batch(&doomed);
        self.attach_to_bones(hung);
        self.bind_bone_refs(refs);
    }

    /// Warn about what removing `owner`'s bone `name` deletes from it.
    fn warn_dropped_bone(&self, owner: u32, bone: u32, name: &str) {
        let Some(doc) = self.world.entity_document(bone) else {
            return;
        };
        if doc.rigidbody.is_some() || doc.joint.is_some() {
            log::warn!(
                "[Asset] entity {owner}: the re-imported model has no bone '{name}' any \
                 more; its Rigidbody / Joint (its ragdoll) were dropped."
            );
        }
        if carries_components(&doc) {
            log::warn!(
                "[Asset] entity {owner}: the re-imported model has no bone '{name}' any \
                 more; the components it carried were deleted with it."
            );
        }
    }
}
