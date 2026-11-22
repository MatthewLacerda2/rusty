use glam::{Mat3, Quat, Vec3};
use rapier3d::prelude::*;

use super::super::convert::to_pose;
use super::super::world::PhysicsWorld;
use rusty_components::{JointComponent, JointKind};

impl PhysicsWorld {
    pub fn joint_world_frame(&self, id: u32) -> Option<Quat> {
        let joint = self.impulse_joints.get(self.joints.get(&id)?.handle)?;
        let body = self.bodies.get(joint.body1())?;
        Some(
            (*body.position() * joint.data.local_frame1)
                .rotation
                .normalize(),
        )
    }
}

pub(super) fn local_frame((bpos, brot): (Vec3, Quat), pos: Vec3, rot: Quat) -> Pose {
    let inv = brot.inverse();
    to_pose(inv * (pos - bpos), (inv * rot).normalize())
}

pub(super) fn joint_basis(axis: Vec3, swing_axis: Vec3) -> Quat {
    let x = axis.try_normalize().unwrap_or(Vec3::X);
    let y = (swing_axis - x * swing_axis.dot(x))
        .try_normalize()
        .unwrap_or_else(|| x.any_orthonormal_vector());
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

pub(super) fn joint_builder(j: &JointComponent) -> GenericJointBuilder {
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
            let swing1 = [-rad(j.swing_limit), rad(j.swing_limit)];
            let swing2 = [-rad(j.swing2_limit), rad(j.swing2_limit)];
            match j.use_limits {
                true => b
                    .limits(JointAxis::AngX, twist)
                    .limits(JointAxis::AngY, swing1)
                    .limits(JointAxis::AngZ, swing2),
                false => b,
            }
        }
    }
}
