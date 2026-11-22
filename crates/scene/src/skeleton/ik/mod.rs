mod aim;
mod two_bone;

pub use aim::{solve_aim, AimSettings};
pub use two_bone::solve_two_bone;

use glam::{Quat, Vec3};

use crate::Scene;
use rusty_components::transform::mirror;
use rusty_components::{IkChain, IkConstraint, IkTarget};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonePose {
    pub pos: Vec3,
    pub rot: Quat,
    pub mirror: Vec3,
}

impl Scene {
    pub fn solve_ik(&mut self) {
        let blended = self.unblend_ragdolls();
        self.solve_constraints();
        self.reblend_ragdolls(blended);
    }

    fn solve_constraints(&mut self) {
        for owner in self.world.ids_with_animator() {
            if !self.world.is_active_in_hierarchy(owner) {
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
        for (&bone, world_rot) in bones.iter().zip(solved.unwrap_or_default()) {
            if let Some(write) = self.set_world_rotation(bone, world_rot) {
                c.applied.push(write);
            }
        }
    }

    fn ik_point(&self, target: IkTarget) -> Option<Vec3> {
        match target {
            IkTarget::Point(p) => Some(p),
            IkTarget::Entity(id) => self
                .world
                .contains(id)
                .then(|| self.world.world_position(id)),
        }
    }

    fn is_simulated(&self, bone: u32) -> bool {
        let dynamic = self
            .world
            .rigidbody(bone)
            .is_some_and(|rb| rb.active && !rb.is_kinematic);
        dynamic && self.ragdolls.weight(bone) >= 1.0
    }

    fn bone_pose(&self, bone: u32) -> BonePose {
        let Some(pose) = self.world.world_pose(bone) else {
            return BonePose {
                pos: Vec3::ZERO,
                rot: Quat::IDENTITY,
                mirror: Vec3::ONE,
            };
        };
        BonePose {
            pos: pose.pos,
            rot: pose.rot,
            mirror: mirror(pose.scale),
        }
    }

    fn set_world_rotation(&mut self, bone: u32, rot: Quat) -> Option<(u32, Quat, Quat)> {
        let local = self.world.local_rotation_for(bone, rot);
        let mut t = self.world.transform_mut(bone)?;
        let before = std::mem::replace(&mut t.rotation, local);
        Some((bone, local, before))
    }
}

#[cfg(test)]
mod aim_tests;
#[cfg(test)]
mod two_bone_tests;
