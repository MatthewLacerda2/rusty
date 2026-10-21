//! src/scene/skeleton/ik/ — inverse kinematics over the bone GameObjects (#461).
//!
//! Each animator's IK constraints (`AnimatorComponent::ik`) are solved once per
//! fixed step by [`Scene::solve_ik`], which the `solve_ik` play system runs
//! **after `LateUpdate` scripts and before `follow_bones`**: the Animator poses the
//! bones, `LateUpdate` sets this tick's targets (and may hand-pose bones), IK
//! bends the result, hitboxes follow the IK'd pose, and the skin palette is built
//! from it in `Render`. A bone carried by a dynamic `Rigidbody` (a ragdoll, #466)
//! belongs to physics, so a constraint touching one is skipped: physics wins.
//!
//! Aim chains solve before limbs (Unity's order: look-at and body, then hands
//! and feet), so a support hand reaching for a foregrip on a gun held by the
//! aimed arm reaches where the aim put it. The solvers (`two_bone`, `aim`) are
//! pure functions over world-space bone poses; this module reads the poses from
//! the scene and writes the results back as bone-local rotations.

mod aim;
mod two_bone;

pub use aim::{solve_aim, AimSettings};
pub use two_bone::solve_two_bone;

use glam::{Quat, Vec3};

use crate::components::{IkChain, IkConstraint, IkTarget};
use crate::scene::Scene;

/// A bone's world position and rotation, and the reflection its world scale
/// carries ([`World::world_mirror`](crate::ecs::World::world_mirror)): its world matrix draws a local direction `d` along
/// `rot · mirror · d` (#1125).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonePose {
    pub pos: Vec3,
    pub rot: Quat,
    /// `±1` per axis; all `+1` for a bone nothing mirrors.
    pub mirror: Vec3,
}

impl Scene {
    /// Solve every active animator's IK constraints, in id order (deterministic).
    pub fn solve_ik(&mut self) {
        for owner in self.world.ids_with_animator() {
            if !self.world.is_active(owner) {
                continue;
            }
            let Some(mut constraints) = self
                .world
                .animator_mut(owner)
                .map(|mut a| std::mem::take(&mut a.ik))
            else {
                continue;
            };
            if constraints.is_empty() {
                continue;
            }
            let mut order: Vec<usize> = (0..constraints.len()).collect();
            order.sort_by_key(|&i| !constraints[i].is_aim());
            for &i in order.iter().rev() {
                self.undo_ik(&mut constraints[i]);
            }
            for &i in &order {
                self.apply_ik(owner, &mut constraints[i]);
            }
            if let Some(mut anim) = self.world.animator_mut(owner) {
                anim.ik = constraints;
            }
        }
    }

    /// Put back what `c`'s last solve wrote on every bone nothing has rewritten
    /// since (the Animator re-poses the bones its clips key; the rest would
    /// otherwise keep compounding IK).
    pub fn undo_ik(&mut self, c: &mut IkConstraint) {
        for (bone, written, before) in c.applied.drain(..).rev() {
            if self
                .world
                .transform(bone)
                .is_some_and(|t| t.rotation == written)
            {
                if let Some(mut t) = self.world.transform_mut(bone) {
                    t.rotation = before;
                }
            }
        }
    }

    fn apply_ik(&mut self, owner: u32, c: &mut IkConstraint) {
        let Some(target) = c.target.and_then(|t| self.ik_point(t)) else {
            return;
        };
        let bones: Option<Vec<u32>> = c
            .bone_names()
            .into_iter()
            .map(|name| self.find_bone(owner, name))
            .collect();
        let Some(bones) = bones.filter(|b| !b.is_empty()) else {
            return;
        };
        if bones.iter().any(|&b| self.is_simulated(b)) {
            return;
        }
        let poses: Vec<BonePose> = bones.iter().map(|&b| self.bone_pose(b)).collect();
        let solved = match &c.chain {
            IkChain::TwoBone { .. } => {
                let hint = c.hint.and_then(|h| self.ik_point(h));
                solve_two_bone(poses[0], poses[1], poses[2].pos, target, hint, c.weight)
                    .map(|(root, mid)| vec![root, mid])
            }
            IkChain::Aim {
                weights,
                axis,
                clamp_degrees,
                ..
            } => {
                let settings = AimSettings {
                    weights,
                    axis: *axis,
                    clamp: clamp_degrees.to_radians(),
                    weight: c.weight,
                };
                solve_aim(&poses, target, settings)
            }
        };
        // Root first: each bone's local rotation is taken against its parent's
        // world pose as already re-solved.
        for (&bone, world_rot) in bones.iter().zip(solved.unwrap_or_default()) {
            if let Some(write) = self.set_world_rotation(bone, world_rot) {
                c.applied.push(write);
            }
        }
    }

    /// Where an IK target or hint is this step.
    fn ik_point(&self, target: IkTarget) -> Option<Vec3> {
        match target {
            IkTarget::Point(p) => Some(p),
            IkTarget::Entity(id) => self
                .world
                .contains(id)
                .then(|| self.world.world_position(id)),
        }
    }

    /// A bone driven by a dynamic body (a ragdolled limb) is physics' to pose.
    fn is_simulated(&self, bone: u32) -> bool {
        self.world
            .rigidbody(bone)
            .is_some_and(|rb| rb.active && !rb.is_kinematic)
    }

    fn bone_pose(&self, bone: u32) -> BonePose {
        BonePose {
            pos: self.world.world_position(bone),
            rot: self.world.world_rotation(bone),
            mirror: self.world.world_mirror(bone),
        }
    }

    /// Give `bone` the world rotation `rot` under its parent's current world pose
    /// (seen through the mirror when the parent reflects, #1119); the `(bone,
    /// written, before)` record.
    fn set_world_rotation(&mut self, bone: u32, rot: Quat) -> Option<(u32, Quat, Quat)> {
        let before = self.world.transform(bone)?.rotation;
        self.world.set_world_rotation(bone, rot);
        let local = self.world.transform(bone)?.rotation;
        Some((bone, local, before))
    }
}

#[cfg(test)]
mod aim_tests;
#[cfg(test)]
mod two_bone_tests;
