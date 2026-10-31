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
//! - `reimport`: `Scene::take_skeleton` + `Scene::restore_skeleton` re-bind a live
//!   skeleton to a re-imported rig in place, by bone name (#1252, #1265): a bone
//!   the rig still names keeps its entity; what hung from a dropped one hangs on
//!   by the same rule as a load.
//! - `Scene::destroy_entity` takes a bone's subtree as it takes any entity's,
//!   and its slots stay missing (skinned at the bind pose) until the skeleton is rebuilt.
//! - `palette` builds each skinned mesh's posed palette from the bones.
//! - `hitbox` fits per-bone hitbox colliders and names the bone a hit struck (#464).
//! - `ragdoll` turns those hitboxes into jointed bodies and switches them between
//!   the Animator and physics (#466).
//! - `ik` bends the posed bones toward script-set targets after `LateUpdate`;
//!   bones a ragdoll's dynamic body carries are skipped (#461).
//! - `probe` counts skeleton reads in tests, so a test can pin a system that must
//!   not read one (a scene without a ragdoll, #1617).

mod bone_refs;
mod hang;
mod hitbox;
mod ik;
mod palette;
mod persist;
pub(crate) mod probe;
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
        let owners = self.skinned_ids().into_iter();
        owners.flat_map(|owner| self.live_bones(owner)).collect()
    }

    /// Every skinned entity: one whose mesh lists a skeleton, in insertion order
    /// (`World::ids_with_mesh`'s).
    pub fn skinned_ids(&self) -> Vec<u32> {
        let ids = self.world.ids_with_mesh().into_iter();
        ids.filter(|&id| {
            self.world
                .mesh(id)
                .is_some_and(|m| !m.skeleton.bones.is_empty())
        })
        .collect()
    }

    /// Every live bone and everything under one (its hitbox, the gun in a hand),
    /// ascending: [`Scene::bone_ids`]' subtrees. One walk down each skinned
    /// entity, so no bone climbs to check it is still under its owner — reaching
    /// it from the owner proves that (#1617). At most [`World::len`] steps per
    /// skeleton, so a malformed child cycle ends the walk.
    ///
    /// [`World::len`]: crate::ecs::World::len
    pub fn bone_subtrees(&self) -> BTreeSet<u32> {
        let mut out = BTreeSet::new();
        for owner in self.skinned_ids() {
            let bones = self.listed_bones(owner);
            let mut stack: Vec<(u32, bool)> = Vec::new();
            stack.extend(self.world.children(owner).into_iter().map(|c| (c, false)));
            let mut steps = self.world.len();
            while let Some((id, under)) = stack.pop().filter(|_| steps > 0) {
                steps -= 1;
                if !self.world.contains(id) {
                    continue;
                }
                let hung = under || bones.binary_search(&id).is_ok();
                if hung {
                    out.insert(id);
                }
                stack.extend(self.world.children(id).into_iter().map(|c| (c, hung)));
            }
        }
        out
    }

    /// The bone ids `owner`'s skeleton lists (present slots), sorted.
    fn listed_bones(&self, owner: u32) -> Vec<u32> {
        probe::read();
        let Some(mesh) = self.world.mesh(owner) else {
            return Vec::new();
        };
        let mut bones: Vec<u32> = mesh.skeleton.bones.iter().flatten().copied().collect();
        bones.sort_unstable();
        bones
    }

    /// `owner`'s bones that are still its bones, with their slots: present (not
    /// missing) and under `owner` ([`Scene::is_under`]). The one walk every reader
    /// that treats a slot as a bone of the skeleton goes through.
    pub(super) fn live_slots(&self, owner: u32) -> Vec<(usize, u32)> {
        probe::read();
        let Some(mesh) = self.world.mesh(owner) else {
            return Vec::new();
        };
        let slots = mesh.skeleton.bones.iter().enumerate();
        let present = slots.filter_map(|(s, b)| Some((s, (*b)?)));
        present.filter(|&(_, b)| self.is_under(b, owner)).collect()
    }

    /// [`Scene::live_slots`]' bones alone.
    fn live_bones(&self, owner: u32) -> impl Iterator<Item = u32> {
        self.live_slots(owner).into_iter().map(|(_, b)| b)
    }

    /// Whether `bone`, listed by `owner`'s skeleton, is still one of its bones:
    /// live and under `owner`. A bone a script moved out from under it (under a
    /// prop, say) is an ordinary entity to the scene (destroy, save, re-import),
    /// though its slot still skins by it; a destroyed one is a missing slot.
    pub(super) fn is_under(&self, bone: u32, owner: u32) -> bool {
        self.world.ancestors(bone).any(|a| a == owner)
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
            let mesh = self.world.mesh(c);
            mesh.inspect(|_| probe::read())
                .is_some_and(|m| m.skeleton.bones.contains(&Some(id)))
        })
    }

    /// `bones` and everything under them, walked from the top ones only: the rest
    /// are their descendants.
    pub(super) fn subtrees(&self, bones: BTreeSet<u32>) -> BTreeSet<u32> {
        bones
            .iter()
            .filter(|&&b| self.world.contains(b))
            .filter(|&&b| !self.world.parent_id(b).is_some_and(|p| bones.contains(&p)))
            .flat_map(|&b| self.world.self_and_descendants(b))
            .collect()
    }

    /// The bone of `owner`'s skeleton named `name` (the glTF joint name), present
    /// in its slots, as Unity's `SkinnedMeshRenderer.bones` keeps a bone Transform
    /// a script moved elsewhere: `Animator.GetBone` reads it.
    pub fn find_bone(&self, owner: u32, name: &str) -> Option<u32> {
        let bones = self.world.mesh(owner)?.skeleton.bones.clone();
        bones
            .into_iter()
            .flatten()
            .find(|&b| self.world.name(b).is_some_and(|n| *n == name))
    }

    /// [`Scene::find_bone`] among the bones still under `owner` only
    /// ([`Scene::live_slots`]): where a stamp hangs an attachment or re-binds a
    /// reference by name, never onto a bone a script moved away.
    pub(super) fn live_bone(&self, owner: u32, name: &str) -> Option<u32> {
        let slots = self.live_slots(owner).into_iter();
        slots
            .map(|(_, b)| b)
            .find(|&b| self.world.name(b).is_some_and(|n| *n == name))
    }
}

#[cfg(test)]
mod bone_refs_tests;
#[cfg(test)]
mod connected_bone_tests;
#[cfg(test)]
mod despawn_refs_tests;
#[cfg(test)]
mod destroy_bone_tests;
#[cfg(test)]
mod drop_warn_tests;
#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod missing_slot_tests;
#[cfg(test)]
mod moved_bone_tests;
#[cfg(test)]
mod persist_tests;
#[cfg(test)]
mod prefab;
#[cfg(test)]
mod ragdoll_refs_tests;
#[cfg(test)]
mod rebind_tests;
#[cfg(test)]
mod reimport_tests;
#[cfg(test)]
mod tests;
