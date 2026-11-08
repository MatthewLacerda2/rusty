//! src/physics/joints/drive.rs — joint drives: motors toward a target pose (#1895).
//!
//! Before each step, [`PhysicsWorld::drive_joints`] sets the angular motors of
//! every joint whose own body is dynamic and whose drive is on: a running hit
//! reaction's drive (`scene.ragdolls.drive_override`) over the joint's own
//! `drive_strength` / `drive_damping`. Unity's `ConfigurableJoint` slerp drive
//! toward `targetRotation`, Unreal's physical animation in local space. The motors
//! are rapier's position motors, acceleration-based: the strength is scaled by
//! the two joined bodies' mass, not by the bodies hanging further down the
//! chain, so a joint carrying a limb is softer than its number says.
//!
//! **The target.** A ragdoll bone with an animated pose (`RagdollRuntime`) is
//! driven toward the rotation its Animator gives it relative to the parent body:
//! the animated local rotations from the parent body down to the bone. Any other
//! joint is driven toward its rest pose (the pose it was built in). rapier
//! measures a joint's angle as `frame1⁻¹ · frame2`, per axis the sine of half the
//! angle, so the target is that rotation for the animated pose, turned into the
//! three angles rapier's motors take; the axes a joint's kind locks drop out.
//!
//! A joint whose body is kinematic, or whose drive is off, carries no motor, so a
//! scene without drives pays one body-type check per joint here.

use glam::Quat;
use rapier3d::prelude::*;

use super::super::compound::body_owner;
use super::super::world::PhysicsWorld;
use super::BuiltJoint;
use crate::scene::skeleton::Drive;
use crate::scene::Scene;

impl PhysicsWorld {
    /// Set this step's motors on the driven joints and take them off the joints
    /// driven last step that no longer are. Ascending joint order.
    pub(in crate::physics) fn drive_joints(&mut self, scene: &Scene) {
        let mut driven = std::collections::BTreeSet::new();
        for (&id, built) in &self.joints {
            let dynamic = self
                .bodies
                .get(built.key.body1)
                .is_some_and(|b| b.is_dynamic());
            let drive = dynamic.then(|| drive_of(scene, id)).flatten();
            let Some(drive) = drive.filter(|d| d.strength > 0.0) else {
                continue;
            };
            let Some(live) = self.impulse_joints.get_mut(built.handle, true) else {
                continue;
            };
            let q = target(scene, id, built);
            let q = if q.w < 0.0 { -q } else { q };
            let angle = |s: f32| 2.0 * s.clamp(-1.0, 1.0).asin();
            for (axis, s) in [
                (JointAxis::AngX, q.x),
                (JointAxis::AngY, q.y),
                (JointAxis::AngZ, q.z),
            ] {
                live.data
                    .set_motor_position(axis, angle(s), drive.strength, drive.damping);
            }
            driven.insert(id);
        }
        for id in self.driven.difference(&driven) {
            let Some(built) = self.joints.get(id) else {
                continue; // dropped or broken: its motors went with it
            };
            if let Some(live) = self.impulse_joints.get_mut(built.handle, true) {
                live.data.motor_axes = JointAxesMask::empty();
            }
        }
        self.driven = driven;
    }
}

/// The drive on joint entity `id`: a reaction's over the component's.
fn drive_of(scene: &Scene, id: u32) -> Option<Drive> {
    if let Some(drive) = scene.ragdolls.drive_override(id) {
        return Some(drive);
    }
    let j = scene.world.joint(id)?;
    Some(Drive {
        strength: j.drive_strength,
        damping: j.drive_damping,
    })
}

/// The joint's target angle error, `frame1⁻¹ · frame2` in frame 1's axes: the
/// rest pose (identity) unless its bone has an animated pose to reach.
fn target(scene: &Scene, id: u32, built: &BuiltJoint) -> Quat {
    let [f1, f2] = built.frames;
    animated_turn(scene, id).map_or(Quat::IDENTITY, |turn| {
        (f1.rotation.inverse() * turn.inverse() * f2.rotation).normalize()
    })
}

/// The animated rotation of joint entity `id`'s body relative to its connected
/// body: the animated local rotations from below the connected body's owner
/// down to `id`. `None` when `id` has no animated pose or no connected body
/// above it.
fn animated_turn(scene: &Scene, id: u32) -> Option<Quat> {
    let (_, mut turn) = scene.ragdolls.animated(id)?;
    let top = body_owner(scene, scene.world.joint(id)?.connected_body?);
    let local = |e: u32| match scene.ragdolls.animated(e) {
        Some((_, rot)) => Some(rot),
        None => scene.world.transform(e).map(|t| t.rotation),
    };
    let mut up = scene.world.parent_id(id);
    while let Some(e) = up {
        if e == top {
            return Some(turn);
        }
        turn = local(e)? * turn;
        up = scene.world.parent_id(e);
    }
    None
}
