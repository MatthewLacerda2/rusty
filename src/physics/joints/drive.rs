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
//! three angles rapier's motors take, on the axes the joint's kind leaves free.
//!
//! A joint whose body is kinematic, or whose drive is off, carries no motor, so a
//! scene without drives pays one body-type check per joint here. What was set
//! is remembered per joint with its rapier handle, so a joint `resync_joints`
//! rebuilt (motorless) is set again even when its target hasn't moved.

use glam::{Mat4, Quat};
use rapier3d::prelude::*;

use super::super::compound::body_owner;
use super::super::world::PhysicsWorld;
use super::BuiltJoint;
use crate::components::transform::world_rotation;
use crate::scene::skeleton::Drive;
use crate::scene::Scene;

/// What a driven joint's motors were last set to: the target, per free axis
/// the angle rapier takes, and the drive. Equal next step means nothing to set.
pub(in crate::physics) type MotorSet = ([f32; 3], Drive);

/// A driven joint's rapier joint and what its motors were set to. The handle
/// tells a rebuilt joint (`Joint.SetLimits`, a body rebuilt), which comes back
/// from rapier with no motors, from the one the motors were set on.
pub(in crate::physics) type Driven = (ImpulseJointHandle, MotorSet);

impl PhysicsWorld {
    /// Set this step's motors on the driven joints and take them off the joints
    /// driven last step that no longer are. Ascending joint order. A joint is
    /// touched only when its target or drive changed since it was last set, so
    /// a driven body holding still can fall asleep (touching a joint wakes both
    /// its bodies). Returns how many joints it touched.
    pub(in crate::physics) fn drive_joints(&mut self, scene: &mut Scene) -> usize {
        let mut driven = std::collections::BTreeMap::new();
        let mut touched = 0;
        for (&id, built) in &self.joints {
            let dynamic = self
                .bodies
                .get(built.key.body1)
                .is_some_and(|b| b.is_dynamic());
            let drive = dynamic.then(|| drive_of(scene, id)).flatten();
            let Some(drive) = drive.filter(|d| d.strength > 0.0) else {
                continue;
            };
            // However the drive was set (`Ragdoll.SetDrive`, `Joint.SetDriveStrength`,
            // the inspector), a simulated ragdoll bone chases its animated pose.
            scene.track_driven_bone(id);
            let q = target(scene, id, built);
            let q = if q.w < 0.0 { -q } else { q };
            let angle = |s: f32| 2.0 * s.clamp(-1.0, 1.0).asin();
            let set: MotorSet = ([angle(q.x), angle(q.y), angle(q.z)], drive);
            driven.insert(id, (built.handle, set));
            if self.driven.get(&id) == Some(&(built.handle, set)) {
                continue;
            }
            let Some(live) = self.impulse_joints.get_mut(built.handle, true) else {
                continue;
            };
            touched += 1;
            let axes = [JointAxis::AngX, JointAxis::AngY, JointAxis::AngZ];
            for (axis, a) in axes.into_iter().zip(set.0) {
                if live.data.locked_axes.contains(axis.into()) {
                    continue; // rapier would solve a motor row against the lock
                }
                live.data
                    .set_motor_position(axis, a, drive.strength, drive.damping);
            }
        }
        for (id, (handle, _)) in self
            .driven
            .iter()
            .filter(|(id, _)| !driven.contains_key(id))
        {
            let Some(built) = self.joints.get(id).filter(|b| b.handle == *handle) else {
                continue; // dropped, broken or rebuilt: its motors went with it
            };
            if let Some(live) = self.impulse_joints.get_mut(built.handle, true) {
                live.data.motor_axes = JointAxesMask::empty();
                touched += 1;
            }
        }
        self.driven = driven;
        touched
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
/// body: both animated world rotations, `R_top⁻¹ · R_id`, composed from the
/// animated local poses below the connected body's owner `top` the way the
/// engine poses them, a mirroring parent included (`F·R_local·F`, #1119), so a
/// mirrored rig is driven toward the pose it draws. `None` when `id` has no
/// animated pose or no connected body above it.
fn animated_turn(scene: &Scene, id: u32) -> Option<Quat> {
    scene.ragdolls.animated(id)?;
    let top = body_owner(scene, scene.world.joint(id)?.connected_body?);
    let mut chain = vec![id];
    let mut up = scene.world.parent_id(id);
    loop {
        let e = up?;
        if e == top {
            break;
        }
        chain.push(e);
        up = scene.world.parent_id(e);
    }
    let local = |e: u32| {
        let t = scene.world.transform(e)?;
        let (pos, rot) = scene
            .ragdolls
            .animated(e)
            .unwrap_or((t.position, t.rotation));
        Some((
            Mat4::from_scale_rotation_translation(t.scale, rot, pos),
            rot,
        ))
    };
    // Only the mirror and scale above `top` matter to the relative turn, and
    // those the Animator doesn't move: the live parent matrix is enough.
    let above = scene
        .world
        .parent_id(top)
        .map(|p| scene.world.local_to_world(p));
    let (top_m, top_local) = local(top)?;
    let top_rot = above.map_or(top_local, |p| world_rotation(p, top_local));
    let mut parent = above.unwrap_or(Mat4::IDENTITY) * top_m;
    let mut rot = top_rot;
    for &e in chain.iter().rev() {
        let (m, r) = local(e)?;
        rot = world_rotation(parent, r);
        parent *= m;
    }
    Some((top_rot.inverse() * rot).normalize())
}

#[cfg(test)]
#[path = "drive_tests.rs"]
mod tests;
