//! src/scene/skeleton/ — bones are GameObjects (#453).
//!
//! A skinned mesh's skeleton exists in the scene as ordinary child entities of the
//! skinned entity, one per joint, named after the glTF joint node — Unity's model.
//! The Animator writes each bone's local Transform, scripts and physics may move
//! them after it, and the palette build reads their world matrices to skin the mesh.
//! Attaching something to a bone is plain parenting: the gun goes under `hand_r`.
//!
//! The skeleton is intrinsic to the asset, so it is **rebuilt from the model, never
//! saved**:
//! - [`Scene::sync_skeletons`] spawns (or re-binds by name) each skinned mesh's
//!   bones after a load, a prefab stamp or an asset instantiate.
//! - [`strip_bones`] takes them out of a saved document, keeping only what the
//!   designer authored: per-bone transform overrides (on the mesh's
//!   `BoneBinding`) and the entities parented under a bone, re-pointed at the
//!   skinned entity with their bone's name (then its ancestors') in `parent_bone`.
//! - [`take_bone_parents`] + [`Scene::hang_on_skeletons`] put those attachments
//!   back under their bones on load (`hang`). A bone that no longer exists hands
//!   them to its nearest surviving ancestor, with a warning, never silently.
//! - `bone_refs`: any other id leaf naming a bone (a joint, a marker, an override)
//!   is saved and re-bound by the bone's name the same way (#1264).
//! - `reimport`: `Scene::take_skeleton` + `Scene::restore_skeleton` rebuild a live
//!   skeleton from a re-imported rig, keeping what was authored on it (#1252), and
//!   hang its attachments back by the same rule as a load.
//! - `palette` builds each skinned mesh's posed palette from the bones.
//! - `hitbox` fits per-bone hitbox colliders and names the bone a hit struck (#464).
//! - `ragdoll` turns those hitboxes into jointed bodies and switches them between
//!   the Animator and physics (#466).
//! - `ik` bends the posed bones toward script-set targets after `LateUpdate`;
//!   bones a ragdoll's dynamic body carries are skipped (#461).

mod bone_refs;
mod hang;
mod hitbox;
mod ik;
mod palette;
mod persist;
mod ragdoll;
mod reimport;
mod sync;

use std::collections::BTreeSet;

pub use bone_refs::{take_bone_refs, BoneRef};
pub use hang::Hung;
pub use hitbox::{HitboxFit, HitboxOptions, HITBOX_NAME};
pub use ik::{AimSettings, BonePose};
pub use persist::{strip_bones, take_bone_parents};
pub use ragdoll::{RagdollOptions, RAGDOLL_LAYER};

use crate::scene::Scene;

/// The `next_entity_id` a save records: the World's, minus the ids at its top
/// held by bones. Bones are respawned with fresh ids on every load, so without
/// this each load-and-save cycle would grow the saved counter by the bone count —
/// churn in a version-controlled scene file. Reusing a bone's id is safe: nothing
/// saved refers to a bone by id.
pub fn saved_next_id(scene: &Scene) -> u32 {
    let bones = scene.bone_ids();
    let mut next = scene.world.next_id();
    while next > 1 && bones.contains(&(next - 1)) {
        next -= 1;
    }
    next
}

impl Scene {
    /// Every bone entity of every skinned mesh in the scene: the ids
    /// [`Scene::is_bone`] holds for, as one set for a caller that asks about many.
    pub fn bone_ids(&self) -> BTreeSet<u32> {
        self.world
            .ids_with_mesh()
            .into_iter()
            .filter_map(|id| self.world.mesh(id).map(|m| (id, m.skeleton.bones.clone())))
            .flat_map(|(owner, bones)| {
                let under_owner = move |&b: &u32| self.world.ancestors(b).any(|a| a == owner);
                bones.into_iter().filter(under_owner)
            })
            .collect()
    }

    /// Whether `id` is a bone: listed by the skeleton of a skinned entity above it.
    /// The one "is a bone" test (#991); [`Scene::bone_ids`] is its set form.
    pub fn is_bone(&self, id: u32) -> bool {
        self.bone_owner(id).is_some()
    }

    /// The skinned entity whose skeleton `id` is a bone of, found by walking up
    /// the hierarchy; `None` for an entity that is not a bone.
    pub fn bone_owner(&self, id: u32) -> Option<u32> {
        self.world.ancestors(id).find(|&c| {
            self.world
                .mesh(c)
                .is_some_and(|m| m.skeleton.bones.contains(&id))
        })
    }

    /// Despawn `owner`'s bones and everything under them — a skinned entity's
    /// skeleton goes with it — in one batch, dropping whatever pointed at them as
    /// [`Scene::destroy_entity`] does.
    pub(crate) fn despawn_skeleton(&mut self, owner: u32) {
        let Some(bones) = self.world.mesh(owner).map(|m| m.skeleton.bones.clone()) else {
            return;
        };
        // Walk from the top bones only: the rest are their descendants.
        let set: BTreeSet<u32> = bones.into_iter().collect();
        let doomed: BTreeSet<u32> = set
            .iter()
            .filter(|&&b| self.world.contains(b))
            .filter(|&&b| !self.world.parent_id(b).is_some_and(|p| set.contains(&p)))
            .flat_map(|&b| self.world.self_and_descendants(b))
            .collect();
        self.despawn_batch(&doomed);
    }

    /// The bone of `owner`'s skeleton named `name` (the glTF joint name).
    pub fn find_bone(&self, owner: u32, name: &str) -> Option<u32> {
        let bones = self.world.mesh(owner)?.skeleton.bones.clone();
        bones
            .into_iter()
            .find(|&b| self.world.name(b).is_some_and(|n| *n == name))
    }
}

#[cfg(test)]
mod bone_refs_tests;
#[cfg(test)]
mod despawn_refs_tests;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod persist_tests;
#[cfg(test)]
mod prefab_refs_tests;
#[cfg(test)]
mod ragdoll_refs_tests;
#[cfg(test)]
mod reimport_tests;
#[cfg(test)]
mod tests;
