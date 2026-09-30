//! src/physics/joints.rs — `Joint` components as rapier impulse joints (#449).
//!
//! Each tick, before the step, [`PhysicsWorld::resync_joints`] matches the live
//! joints to the scene: a joint is (re)built when it is new, when its shape
//! (kind, bodies, anchors, axis, limits, collision flag) changed, or when either
//! body was rebuilt — rapier drops a removed body's joints with it. Break
//! thresholds are read live and never force a rebuild.
//!
//! **Frames.** The joint's frame sits at `anchor` (this entity's local space,
//! scaled like its geometry) with its X axis along `axis`. Both bodies get that
//! same world frame, so the pose the bodies are in when the joint is built is its
//! rest pose, and the limits are measured from it (Unity's behaviour). With
//! `auto_configure_connected_anchor` off, the connected side's origin moves to
//! `connected_anchor` (the connected entity's local space, or world space with no
//! connected body — which joins to one shared fixed "world" body).
//!
//! **Breaking.** After the step, [`PhysicsWorld::break_joints`] reads each joint's
//! solver impulse. rapier keeps the impulse of one solver *substep*, so force =
//! linear impulse / substep and torque = angular impulse / substep. A
//! joint past a non-zero `break_force` / `break_torque` is removed and its
//! `Joint` component destroyed (Unity), and reported for `OnJointBreak`.
//!
//! **Determinism.** Joints live in a `BTreeMap` keyed by the joint entity's id, so
//! they are built, checked and broken in ascending id order every run.

use std::collections::BTreeMap;

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use super::compound::{body_owner, world_pose};
use super::convert::{from_iso, to_iso};
use super::world::PhysicsWorld;
use crate::components::{JointComponent, JointKind};
use crate::scene::Scene;

/// One joint that broke this tick: the joint entity and what broke it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointBreak {
    /// The entity whose `Joint` broke (its component is gone).
    pub id: u32,
    /// The force (newtons) the joint carried on the breaking tick.
    pub force: f32,
    /// The torque (newton-metres) the joint carried on the breaking tick.
    pub torque: f32,
}

/// What a built joint was built from: its shape and both bodies (`None` is the
/// world). Any difference means a rebuild.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct JointKey {
    shape: JointComponent,
    body1: RigidBodyHandle,
    body2: Option<RigidBodyHandle>,
}

/// A joint live in rapier.
#[derive(Debug)]
pub(super) struct BuiltJoint {
    pub(super) key: JointKey,
    pub(super) handle: ImpulseJointHandle,
}

/// Joint entity id → its live joint.
pub(super) type JointMap = BTreeMap<u32, BuiltJoint>;

/// `j` with the live-read fields (break thresholds) cleared: the part of a joint
/// that decides how it is built.
fn shape_of(j: &JointComponent) -> JointComponent {
    JointComponent {
        break_force: 0.0,
        break_torque: 0.0,
        ..j.clone()
    }
}

impl PhysicsWorld {
    /// Build, rebuild and drop joints so the live set matches the scene.
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

    /// Every buildable joint in the scene, keyed by its entity: an active entity
    /// with a `Joint` whose own body exists, and whose connected body (when it
    /// names one) exists too. A joint missing a body is inert, not an error.
    fn wanted_joints(&self, scene: &Scene) -> BTreeMap<u32, JointKey> {
        let mut out = BTreeMap::new();
        for id in scene.world.ids_with_joint() {
            let Some(joint) = scene.world.joint(id).map(|j| shape_of(&j)) else {
                continue;
            };
            if !scene.world.is_active(id) {
                continue;
            }
            let body = |e: u32| self.id_to_body.get(&body_owner(scene, e)).copied();
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

    /// Insert joint entity `id`'s rapier joint (see the module docs for frames).
    fn build_joint(&mut self, scene: &Scene, id: u32, key: JointKey) -> Option<BuiltJoint> {
        let j = &key.shape;
        let pose = world_pose(scene, id)?;
        let anchor = pose.pos + pose.rot * (pose.scale * j.anchor);
        let rot = pose.rot * Quat::from_rotation_arc(Vec3::X, j.axis.normalize_or_zero());
        let connected = match (j.auto_configure_connected_anchor, j.connected_body) {
            (true, _) => anchor,
            (false, None) => j.connected_anchor,
            (false, Some(other)) => {
                let p = world_pose(scene, other)?;
                p.pos + p.rot * (p.scale * j.connected_anchor)
            }
        };
        let body2 = match key.body2 {
            Some(h) => h,
            None => self.world_body(),
        };
        let frame1 = self.local_frame(key.body1, anchor, rot)?;
        let frame2 = self.local_frame(body2, connected, rot)?;
        let data = joint_builder(j)
            .local_frame1(frame1)
            .local_frame2(frame2)
            .contacts_enabled(j.enable_collision)
            .build();
        let handle = self.impulse_joints.insert(key.body1, body2, data, true);
        Some(BuiltJoint { key, handle })
    }

    /// The world frame `(pos, rot)` expressed in `body`'s local frame.
    fn local_frame(&self, body: RigidBodyHandle, pos: Vec3, rot: Quat) -> Option<Isometry<Real>> {
        let (bpos, brot) = from_iso(self.bodies.get(body)?.position());
        let inv = brot.inverse();
        Some(to_iso(inv * (pos - bpos), (inv * rot).normalize()))
    }

    /// The shared fixed body joints with no connected body attach to, created on
    /// first use at the world origin. It carries no collider.
    fn world_body(&mut self) -> RigidBodyHandle {
        if let Some(h) = self.world_body.filter(|&h| self.bodies.contains(h)) {
            return h;
        }
        let h = self.bodies.insert(RigidBodyBuilder::fixed().build());
        self.world_body = Some(h);
        h
    }

    /// Break every joint whose solver load this tick passed its threshold: drop
    /// the rapier joint, destroy the `Joint` component, and report it. Ascending
    /// joint-entity order.
    pub(super) fn break_joints(&mut self, scene: &mut Scene, dt: f32) -> Vec<JointBreak> {
        let substep = dt / self.integration_parameters.num_solver_iterations.get() as f32;
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
            let force = live.impulses.fixed_rows::<3>(0).norm() / substep;
            let torque = live.impulses.fixed_rows::<3>(3).norm() / substep;
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

/// Whether `load` breaks a joint rated `threshold` (`0`: unbreakable).
fn exceeds(load: f32, threshold: f32) -> bool {
    threshold > 0.0 && load > threshold
}

/// The rapier joint for `j`'s kind: which axes are locked and the angle limits
/// (degrees → radians) on the free ones. The joint X axis is `j.axis`.
fn joint_builder(j: &JointComponent) -> GenericJointBuilder {
    let rad = |d: f32| d.to_radians();
    let twist = [rad(j.limits.x), rad(j.limits.y)];
    match j.kind {
        JointKind::Fixed => GenericJointBuilder::new(JointAxesMask::LOCKED_FIXED_AXES),
        JointKind::Hinge => {
            let b = GenericJointBuilder::new(JointAxesMask::LOCKED_REVOLUTE_AXES);
            match j.use_limits {
                true => b.limits(JointAxis::AngX, twist),
                false => b,
            }
        }
        JointKind::Ball => {
            let b = GenericJointBuilder::new(JointAxesMask::LOCKED_SPHERICAL_AXES);
            let swing = [-rad(j.swing_limit), rad(j.swing_limit)];
            match j.use_limits {
                true => b
                    .limits(JointAxis::AngX, twist)
                    .limits(JointAxis::AngY, swing)
                    .limits(JointAxis::AngZ, swing),
                false => b,
            }
        }
    }
}
