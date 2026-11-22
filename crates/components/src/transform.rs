//! Unity's Transform, and the mirror-aware pose math.

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransformComponent {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl TransformComponent {
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    pub fn from_matrix(m: Mat4) -> Self {
        let (scale, rotation, position) = m.to_scale_rotation_translation();
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn approx_eq(&self, other: &Self, eps: f32) -> bool {
        self.position.abs_diff_eq(other.position, eps)
            && self.scale.abs_diff_eq(other.scale, eps)
            && (self.rotation.abs_diff_eq(other.rotation, eps)
                || self.rotation.abs_diff_eq(-other.rotation, eps))
    }

    pub fn pose(&self) -> Pose {
        Pose {
            pos: self.position,
            rot: self.rotation,
            scale: self.scale,
        }
    }

    pub fn euler_angles(&self) -> Vec3 {
        euler_degrees(self.rotation)
    }

    pub fn set_euler_angles(&mut self, euler_deg: Vec3) {
        self.rotation = from_euler_degrees(euler_deg);
    }
}

/// Degrees `(pitch, yaw, roll)`, applied yaw, pitch, then roll.
pub fn euler_degrees(rotation: Quat) -> Vec3 {
    let (yaw, pitch, roll) = rotation.to_euler(glam::EulerRot::YXZ);
    Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
}

pub fn from_euler_degrees(euler_deg: Vec3) -> Quat {
    let yaw = euler_deg.y.to_radians();
    let pitch = euler_deg.x.to_radians();
    let roll = euler_deg.z.to_radians();
    Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pos: Vec3,
    pub rot: Quat,
    pub scale: Vec3,
}

impl Pose {
    pub fn under(self, parent_world: Mat4) -> Self {
        Self {
            pos: parent_world.transform_point3(self.pos),
            rot: world_rotation(parent_world, self.rot),
            scale: lossy_scale(parent_world, self.rot, self.scale),
        }
    }

    pub fn mirror_frame(&self, x: Vec3, y: Vec3) -> (Vec3, Vec3) {
        mirrored_frame_axes(mirror(self.scale), x, y)
    }
}

/// glam splits a parent's reflection out as a negative X scale, so the child's rotation is seen
/// through that mirror, never multiplied plainly.
pub fn world_rotation(parent_world: Mat4, rotation: Quat) -> Quat {
    let parent_rot = parent_rotation(parent_world);
    (parent_rot * through_mirror(parent_world, rotation)).normalize()
}

pub fn local_rotation(parent_world: Mat4, world_rot: Quat) -> Quat {
    let parent_rot = parent_rotation(parent_world);
    through_mirror(parent_world, parent_rot.inverse() * world_rot).normalize()
}

/// Relative tolerance (a few ulps) for a parent's axes to read equal-length and square.
const UNIFORM: f32 = 4.0 * f32::EPSILON;

fn lossy_scale(parent_world: Mat4, rotation: Quat, scale: Vec3) -> Vec3 {
    let split = parent_world.to_scale_rotation_translation().0;
    let parent = glam::Mat3::from_mat4(parent_world);
    // A turn cannot change a uniform scale, and reading 1000 through the rotation gives 999.9999.
    if conformal(parent, split.abs()) {
        return split * scale;
    }
    let turned = glam::Mat3::from_quat(rotation);
    let along = |u: Vec3| (parent * u).length() / u.length();
    let lengths = Vec3::new(
        along(turned.x_axis),
        along(turned.y_axis),
        along(turned.z_axis),
    );
    let flip = if parent_world.determinant() < 0.0 {
        -1.0
    } else {
        1.0
    };
    Vec3::new(flip, 1.0, 1.0) * lengths * scale
}

fn conformal(m: glam::Mat3, size: Vec3) -> bool {
    let top = size.max_element();
    let square = top * top;
    let (x, y, z) = (m.x_axis, m.y_axis, m.z_axis);
    top - size.min_element() <= UNIFORM * top
        && [x.dot(y), x.dot(z), y.dot(z)]
            .iter()
            .all(|d| d.abs() <= UNIFORM * square)
}

pub fn mirror(signed_scale: Vec3) -> Vec3 {
    Vec3::select(signed_scale.cmplt(Vec3::ZERO), -Vec3::ONE, Vec3::ONE)
}

pub fn mirrored_direction(mirror: Vec3, dir: Vec3) -> Vec3 {
    mirror * dir
}

/// Only the X twist mirrors exactly: fine while swing limits are symmetric, as
/// `physics::joints::frame::joint_builder` sets them.
pub fn mirrored_frame_axes(mirror: Vec3, x: Vec3, y: Vec3) -> (Vec3, Vec3) {
    let sense = mirror.x * mirror.y * mirror.z;
    (sense * (mirror * x), mirror * y)
}

/// A zero scale axis makes glam's rotation NaN, which would poison every pose below.
fn parent_rotation(parent_world: Mat4) -> Quat {
    let (_, rot, _) = parent_world.to_scale_rotation_translation();
    if rot.is_finite() {
        rot
    } else {
        Quat::IDENTITY
    }
}

fn through_mirror(parent_world: Mat4, rotation: Quat) -> Quat {
    if parent_world.determinant() < 0.0 {
        Quat::from_xyzw(rotation.x, -rotation.y, -rotation.z, rotation.w)
    } else {
        rotation
    }
}

#[cfg(test)]
mod tests;
