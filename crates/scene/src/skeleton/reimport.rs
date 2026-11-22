//! A re-imported rig re-binds a live skeleton in place, by bone name, as Unity keeps a
//! model's bone Transforms: a kept bone keeps its entity, children and any override.
use std::collections::{BTreeMap, BTreeSet};

use super::bone_refs::BoneRef;
use super::hang::Hung;
use super::persist::{bone_chain, carries_components, OVERRIDE_EPS};
use super::sync::rest_local;
use crate::{Despawn, Scene};
use rusty_components::TransformComponent;

#[derive(Debug, Default)]
pub struct Parked {
    bones: Vec<(u32, Vec<String>)>,
    /// Keyed by entity, not name, so two joints of one name each keep their own.
    overrides: BTreeMap<u32, TransformComponent>,
}

impl Scene {
    pub fn take_skeleton(&self, owner: u32) -> Parked {
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

    pub fn restore_skeleton(&mut self, owner: u32, parked: Parked) {
        let Parked {
            bones: old,
            mut overrides,
        } = parked;
        let skin = self.world.mesh(owner).and_then(|m| m.skin.clone());
        let mut pool: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
        // Name → its old bones, last slot first, so `pop` takes the first.
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
        // Edit mode only: in Play this despawn would skip script teardown.
        self.despawn_batch(&doomed, Despawn::EditMode);
        self.attach_to_bones(hung, Despawn::EditMode);
        self.bind_bone_refs(refs);
    }

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
