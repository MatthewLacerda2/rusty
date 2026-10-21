//! src/components/transform.rs — Transform component (mandatory).
//!
//! Every entity has exactly one Transform (position/rotation/scale). Unity:
//! Transform. Moved verbatim from the legacy `core/scene.rs`.

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransformComponent {
    pub position: Vec3,
    pub rotation: Quat, // We will also support Euler representation in UI
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

    /// Decompose an affine matrix (no shear) into position / rotation / scale.
    pub fn from_matrix(m: Mat4) -> Self {
        let (scale, rotation, position) = m.to_scale_rotation_translation();
        Self {
            position,
            rotation,
            scale,
        }
    }

    /// Whether `other` is the same transform within `eps` (a rotation and its
    /// negation are the same orientation).
    pub fn approx_eq(&self, other: &Self, eps: f32) -> bool {
        self.position.abs_diff_eq(other.position, eps)
            && self.scale.abs_diff_eq(other.scale, eps)
            && (self.rotation.abs_diff_eq(other.rotation, eps)
                || self.rotation.abs_diff_eq(-other.rotation, eps))
    }

    pub fn euler_angles(&self) -> Vec3 {
        let (yaw, pitch, roll) = self.rotation.to_euler(glam::EulerRot::YXZ);
        Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
    }

    pub fn set_euler_angles(&mut self, euler_deg: Vec3) {
        let yaw = euler_deg.y.to_radians();
        let pitch = euler_deg.x.to_radians();
        let roll = euler_deg.z.to_radians();
        self.rotation = Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll);
    }
}

/// The world rotation of a child with local `rotation` under a parent whose world
/// matrix is `parent_world`. A reflection stays in the scale, never in the
/// rotation: glam's `to_scale_rotation_translation` carries a mirroring parent's
/// reflection as a negative X scale, so the child's rotation is seen through that
/// mirror, `R_parent · F·R·F` with `F` the X flip, not the plain product (#1119).
/// Then this rotation times the parent's signed scale times the child's own
/// scale is the child's world matrix, which is how physics poses a body and IK a
/// bone. [`local_rotation`] is its exact inverse; an unmirrored parent gives the
/// plain product, bit for bit.
pub fn world_rotation(parent_world: Mat4, rotation: Quat) -> Quat {
    let (_, parent_rot, _) = parent_world.to_scale_rotation_translation();
    (parent_rot * through_mirror(parent_world, rotation)).normalize()
}

/// The local rotation that gives a child the world rotation `world_rot` under
/// `parent_world`: the inverse of [`world_rotation`], so a world rotation read
/// from a bone and written back leaves its local pose as it was, mirrored parent
/// or not (#1119).
pub fn local_rotation(parent_world: Mat4, world_rot: Quat) -> Quat {
    let (_, parent_rot, _) = parent_world.to_scale_rotation_translation();
    through_mirror(parent_world, parent_rot.inverse() * world_rot).normalize()
}

/// A direction written in a child's local space, as the child's
/// [`world_rotation`] must turn it to land on the drawn world direction: `F·dir`
/// under a mirroring parent, `dir` itself otherwise. The child's matrix draws a
/// local `dir` along `R_w·F·dir` (the reflection lives in the scale), so a local
/// aim axis or joint axis is mirrored once, here, before the world rotation
/// turns it (#1125). An unmirrored parent returns `dir` bit for bit.
pub fn mirrored_direction(parent_world: Mat4, dir: Vec3) -> Vec3 {
    if mirrors(parent_world) {
        Vec3::new(-dir.x, dir.y, dir.z)
    } else {
        dir
    }
}

/// Whether `parent_world` reflects (a negative determinant).
fn mirrors(parent_world: Mat4) -> bool {
    parent_world.determinant() < 0.0
}

/// `rotation` seen through a mirroring parent's reflection, `F·R·F`: a rotation
/// about the axis reflected as a pseudovector, `(x, y, z, w) → (x, −y, −z, w)`.
/// Its own inverse, so one function serves both directions.
fn through_mirror(parent_world: Mat4, rotation: Quat) -> Quat {
    if mirrors(parent_world) {
        Quat::from_xyzw(rotation.x, -rotation.y, -rotation.z, rotation.w)
    } else {
        rotation
    }
}

#[cfg(test)]
mod tests;
