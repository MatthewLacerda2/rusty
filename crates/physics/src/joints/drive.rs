use glam::{Mat4, Quat};
use rapier3d::prelude::*;

use super::super::compound::body_owner;
use super::super::world::PhysicsWorld;
use super::BuiltJoint;
use rusty_components::transform::world_rotation;
use rusty_scene::skeleton::Drive;
use rusty_scene::Scene;

pub(crate) type MotorSet = ([f32; 3], Drive);

pub(crate) type Driven = (ImpulseJointHandle, MotorSet);

impl PhysicsWorld {
    pub(crate) fn drive_joints(&mut self, scene: &mut Scene) -> usize {
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
                continue;
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

fn target(scene: &Scene, id: u32, built: &BuiltJoint) -> Quat {
    let [f1, f2] = built.frames;
    animated_turn(scene, id).map_or(Quat::IDENTITY, |turn| {
        (f1.rotation.inverse() * turn.inverse() * f2.rotation).normalize()
    })
}

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
