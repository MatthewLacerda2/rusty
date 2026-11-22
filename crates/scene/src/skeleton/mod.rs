//! Bones are GameObjects, rebuilt from the model on every load and never saved.
mod bone_refs;
mod hang;
mod hitbox;
mod ik;
mod palette;
mod persist;
#[doc(hidden)]
pub mod probe;
mod ragdoll;
mod reimport;
mod sync;

use std::collections::BTreeSet;

pub use bone_refs::{take_bone_refs, BoneRef};
pub use hang::Hung;
pub use hitbox::{HitboxFit, HitboxOptions, HITBOX_NAME};
pub use ik::{AimSettings, BonePose};
pub use persist::{strip_bones, take_bone_parents};
pub use ragdoll::{
    critical_damping, Drive, HitReact, RagdollOptions, RagdollRuntime, RAGDOLL_LAYER,
    REACT_DURATION, REACT_STRENGTH,
};

use crate::Scene;

/// The World's next id less the bones at its top: bones respawn with fresh ids on each
/// load, so otherwise every load and save would grow the saved counter.
pub fn saved_next_id(scene: &Scene) -> u32 {
    let bones = scene.bone_ids();
    let mut next = scene.world.next_id();
    while next > 1 && bones.contains(&(next - 1)) {
        next -= 1;
    }
    next
}

impl Scene {
    pub fn bone_ids(&self) -> BTreeSet<u32> {
        let owners = self.skinned_ids().into_iter();
        owners.flat_map(|owner| self.live_bones(owner)).collect()
    }

    pub fn skinned_ids(&self) -> Vec<u32> {
        self.world
            .ids_with_mesh_where(|m| !m.skeleton.bones.is_empty())
    }

    /// Reaching a bone from its owner proves it is still under it; the step budget
    /// ends a malformed child cycle.
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

    fn listed_bones(&self, owner: u32) -> Vec<u32> {
        probe::read();
        let Some(mesh) = self.world.mesh(owner) else {
            return Vec::new();
        };
        let mut bones: Vec<u32> = mesh.skeleton.bones.iter().flatten().copied().collect();
        bones.sort_unstable();
        bones
    }

    pub(super) fn live_slots(&self, owner: u32) -> Vec<(usize, u32)> {
        probe::read();
        let Some(mesh) = self.world.mesh(owner) else {
            return Vec::new();
        };
        let slots = mesh.skeleton.bones.iter().enumerate();
        let present = slots.filter_map(|(s, b)| Some((s, (*b)?)));
        present.filter(|&(_, b)| self.is_under(b, owner)).collect()
    }

    fn live_bones(&self, owner: u32) -> impl Iterator<Item = u32> {
        self.live_slots(owner).into_iter().map(|(_, b)| b)
    }

    pub(super) fn is_under(&self, bone: u32, owner: u32) -> bool {
        self.world.ancestors(bone).any(|a| a == owner)
    }

    pub fn is_bone(&self, id: u32) -> bool {
        self.bone_owner(id).is_some()
    }

    pub fn bone_owner(&self, id: u32) -> Option<u32> {
        self.world.ancestors(id).find(|&c| {
            let mesh = self.world.mesh(c);
            mesh.inspect(|_| probe::read())
                .is_some_and(|m| m.skeleton.bones.contains(&Some(id)))
        })
    }

    pub(super) fn subtrees(&self, bones: BTreeSet<u32>) -> BTreeSet<u32> {
        bones
            .iter()
            .filter(|&&b| self.world.contains(b))
            .filter(|&&b| !self.world.parent_id(b).is_some_and(|p| bones.contains(&p)))
            .flat_map(|&b| self.world.self_and_descendants(b))
            .collect()
    }

    /// Any present slot, as Unity keeps a bone a script moved away: `Animator.GetBone`.
    pub fn find_bone(&self, owner: u32, name: &str) -> Option<u32> {
        let bones = self.world.mesh(owner)?.skeleton.bones.clone();
        bones
            .into_iter()
            .flatten()
            .find(|&b| self.world.name(b).is_some_and(|n| *n == name))
    }

    /// Only bones still under `owner`: a stamp never hangs or re-binds onto a moved one.
    pub(super) fn live_bone(&self, owner: u32, name: &str) -> Option<u32> {
        let slots = self.live_slots(owner).into_iter();
        slots
            .map(|(_, b)| b)
            .find(|&b| self.world.name(b).is_some_and(|n| *n == name))
    }
}

#[cfg(test)]
mod drop_warn_tests;
#[doc(hidden)]
pub mod rig;
#[cfg(test)]
mod skinned_tests;
