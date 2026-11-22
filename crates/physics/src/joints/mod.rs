use std::collections::BTreeMap;

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use super::compound::{body_owner, world_pose};
use super::events::collision::VelocitySnapshot;
use super::world::PhysicsWorld;
use frame::{joint_basis, joint_builder, local_frame};
use rusty_components::JointComponent;
use rusty_scene::Scene;

#[cfg(test)]
mod disabled_tests;
pub(super) mod drive;
mod frame;
#[cfg(test)]
mod frame_tests;
#[cfg(test)]
mod impulse_pair_tests;
#[cfg(test)]
mod impulse_tests;
#[cfg(test)]
mod inert_tests;
mod load;
#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod mirrored_tests;
pub(super) mod ragdoll;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointBreak {
    pub id: u32,
    pub force: f32,
    pub torque: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct JointKey {
    shape: JointComponent,
    body1: RigidBodyHandle,
    body2: Option<RigidBodyHandle>,
}

#[derive(Debug)]
pub(super) struct BuiltJoint {
    pub(super) key: JointKey,
    pub(super) handle: ImpulseJointHandle,
    pub(super) frames: [Pose; 2],
}

pub(super) type JointMap = BTreeMap<u32, BuiltJoint>;

fn shape_of(j: &JointComponent) -> JointComponent {
    JointComponent {
        break_force: 0.0,
        break_torque: 0.0,
        drive_strength: 0.0,
        drive_damping: 0.0,
        ..j.clone()
    }
}

impl PhysicsWorld {
    pub(super) fn resync_joints(&mut self, scene: &Scene) {
        let wanted = self.wanted_joints(scene);
        let stale: Vec<u32> = self
            .joints
            .iter()
            .filter(|(id, built)| wanted.get(id) != Some(&built.key))
            .map(|(&id, _)| id)
            .collect();
        for id in stale {
            if let Some(built) = self.joints.remove(&id) {
                self.impulse_joints.remove(built.handle, true);
            }
        }
        for (id, key) in wanted {
            if !self.joints.contains_key(&id) {
                if let Some(built) = self.build_joint(scene, id, key) {
                    self.joints.insert(id, built);
                }
            }
        }
    }

    ///: rapier picks joints for its solver without checking `is_enabled`,
    /// so a body built disabled joins one as a second world-attached side and a
    /// fixed partner panics its solver (`unreachable!()`), and one disabled later
    /// keeps a stale solver slot.
    fn wanted_joints(&self, scene: &Scene) -> BTreeMap<u32, JointKey> {
        let mut out = BTreeMap::new();
        for id in scene.world.ids_with_joint() {
            let Some(joint) = scene.world.joint(id).map(|j| shape_of(&j)) else {
                continue;
            };
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let body = |e: u32| {
                let owner = body_owner(scene, e);
                let enabled = scene.world.is_active_in_hierarchy(owner);
                self.id_to_body.get(&owner).copied().filter(|_| enabled)
            };
            let Some(body1) = body(id) else {
                continue;
            };
            let body2 = match joint.connected_body {
                Some(other) => match body(other) {
                    Some(h) if h != body1 => Some(h),
                    _ => continue,
                },
                None => None,
            };
            let key = JointKey {
                shape: joint,
                body1,
                body2,
            };
            out.insert(id, key);
        }
        out
    }

    fn build_joint(&mut self, scene: &Scene, id: u32, key: JointKey) -> Option<BuiltJoint> {
        let j = &key.shape;
        let pose = world_pose(scene, id)?;
        let anchor = pose.pos + pose.rot * (pose.scale * j.anchor);
        let (axis, swing) = pose.mirror_frame(j.axis, j.swing_axis);
        let rot = pose.rot * joint_basis(axis, swing);
        let connected = match (j.auto_configure_connected_anchor, j.connected_body) {
            (true, _) => anchor,
            (false, None) => j.connected_anchor,
            (false, Some(other)) => {
                let p = world_pose(scene, other)?;
                p.pos + p.rot * (p.scale * j.connected_anchor)
            }
        };
        // Both frames come from the scene's poses (the bodies' owners), not the
        // rapier bodies, which lag a script's move of a kinematic body by a tick.
        let owner_pose = |e: u32| world_pose(scene, body_owner(scene, e)).map(|p| (p.pos, p.rot));
        let frame1 = local_frame(owner_pose(id)?, anchor, rot);
        let (body2, pose2) = match (key.body2, j.connected_body) {
            (Some(h), Some(other)) => (h, owner_pose(other)?),
            _ => (self.world_body(), (Vec3::ZERO, Quat::IDENTITY)),
        };
        let frame2 = local_frame(pose2, connected, rot);
        let data = joint_builder(j)
            .local_frame1(frame1)
            .local_frame2(frame2)
            .contacts_enabled(j.enable_collision)
            .build();
        let handle = self.impulse_joints.insert(key.body1, body2, data, true);
        let frames = [frame1, frame2];
        Some(BuiltJoint {
            key,
            handle,
            frames,
        })
    }

    pub(super) fn keep_joint_frames(&mut self, old: &JointMap) {
        for (id, built) in &mut self.joints {
            let same = |was: &&BuiltJoint| {
                was.key.shape == built.key.shape
                    && was.key.body2.is_some() == built.key.body2.is_some()
            };
            let Some(was) = old.get(id).filter(same) else {
                continue;
            };
            built.frames = was.frames;
            if let Some(live) = self.impulse_joints.get_mut(built.handle, false) {
                let [frame1, frame2] = was.frames;
                live.data.set_local_frame1(frame1).set_local_frame2(frame2);
            }
        }
    }

    fn world_body(&mut self) -> RigidBodyHandle {
        if let Some(h) = self.world_body.filter(|&h| self.bodies.contains(h)) {
            return h;
        }
        let h = self.insert_body(RigidBodyBuilder::fixed().build());
        self.world_body = Some(h);
        h
    }

    pub(super) fn break_joints(
        &mut self,
        scene: &mut Scene,
        pre: &VelocitySnapshot,
        dt: f32,
    ) -> Vec<JointBreak> {
        let substep = dt / self.integration_parameters.num_solver_iterations as f32;
        if substep <= 0.0 {
            return Vec::new();
        }
        let mut broken = Vec::new();
        for (&id, built) in &self.joints {
            let (Some(joint), Some(live)) = (
                scene.world.joint(id).map(|j| j.clone()),
                self.impulse_joints.get(built.handle),
            ) else {
                continue;
            };
            let [fx, fy, fz, ..] = live.impulses;
            let (lin, ang) = load::step_load(live, &self.bodies, pre, self.gravity, dt);
            let force = (Vec3::new(fx, fy, fz).length() / substep).max(lin.length() / dt);
            let torque = load::anchor_torque(live, &self.bodies).length() / substep;
            let torque = torque.max(ang.length() / dt);
            if exceeds(force, joint.break_force) || exceeds(torque, joint.break_torque) {
                broken.push(JointBreak { id, force, torque });
            }
        }
        for b in &broken {
            if let Some(built) = self.joints.remove(&b.id) {
                self.impulse_joints.remove(built.handle, true);
            }
            scene.world.set_joint(b.id, None);
        }
        broken
    }
}

fn exceeds(load: f32, threshold: f32) -> bool {
    threshold > 0.0 && load > threshold
}
