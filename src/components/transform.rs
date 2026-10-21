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

    /// This transform as a [`Pose`], in its parent's space.
    pub fn pose(&self) -> Pose {
        Pose {
            pos: self.position,
            rot: self.rotation,
            scale: self.scale,
        }
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

/// A pose split the way physics poses a body: translation, a proper rotation,
/// and a signed (lossy) scale whose signs carry any mirror. A child's world
/// pose is [`Pose::under`] its parent's world matrix, which is how a collider's
/// world AABB and its rapier shape read the same rotation and scale (#1140).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pos: Vec3,
    pub rot: Quat,
    pub scale: Vec3,
}

impl Pose {
    /// This local pose under a parent whose world matrix is `parent_world`:
    /// [`world_rotation`] and `lossy_scale`, never a split of the product
    /// matrix, which loses a rotated child's scale axes.
    pub fn under(self, parent_world: Mat4) -> Self {
        Self {
            pos: parent_world.transform_point3(self.pos),
            rot: world_rotation(parent_world, self.rot),
            scale: lossy_scale(parent_world, self.rot, self.scale),
        }
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
/// plain product, bit for bit. A degenerate parent (a zero scale axis)
/// contributes no rotation (`parent_rotation`), so the result is never NaN.
pub fn world_rotation(parent_world: Mat4, rotation: Quat) -> Quat {
    let parent_rot = parent_rotation(parent_world);
    (parent_rot * through_mirror(parent_world, rotation)).normalize()
}

/// The local rotation that gives a child the world rotation `world_rot` under
/// `parent_world`: the inverse of [`world_rotation`], so a world rotation read
/// from a bone and written back leaves its local pose as it was, mirrored parent
/// or not (#1119).
pub fn local_rotation(parent_world: Mat4, world_rot: Quat) -> Quat {
    let parent_rot = parent_rotation(parent_world);
    through_mirror(parent_world, parent_rot.inverse() * world_rot).normalize()
}

/// A child's signed world scale when it is not turned against its parent:
/// `parent_world`'s split scale (its reflection on X, as glam decomposes it)
/// times the child's own `scale`. Its signs are always the child's [`mirror`],
/// turned or not; its magnitudes are [`Pose::under`]'s for an unturned child or
/// a parent scaled the same on every axis.
pub fn world_scale(parent_world: Mat4, scale: Vec3) -> Vec3 {
    parent_world.to_scale_rotation_translation().0 * scale
}

/// How far apart, relative to the largest, a parent's three scales may be, and how
/// far from square its axes, and still read as uniform in [`lossy_scale`]: a few ulps.
const UNIFORM: f32 = 4.0 * f32::EPSILON;

/// Unity's `lossyScale` of a child with local `rotation` and `scale` under
/// `parent_world`: the lengths of its world matrix's columns, so the parent's
/// scale is read along the child's own axes, however it is turned (#1156). The
/// signs are the mirror convention: the child's own, with X flipped once more
/// when the parent reflects (glam's split carries a reflection on X, and
/// [`world_rotation`] sees through it). They are the child's [`mirror`], an
/// entity's own negative scale included (#1125). Then [`world_rotation`] times
/// this scale is the world matrix whenever the hierarchy does not shear.
///
/// Under a conformal parent, one scaled the same on every axis with its axes still
/// square (`M·Mᵀ = s²·I`, to within [`UNIFORM`], the rounding a matrix carries), it
/// is [`world_scale`] exactly: a turn cannot change a uniform scale, and reading it
/// through the rotation's rounding would (a parent at 1000 would give 999.9999).
/// Equal axis lengths alone are not enough: a turned parent under a non-uniform
/// grandparent can shear into three equal-length axes that are not square.
fn lossy_scale(parent_world: Mat4, rotation: Quat, scale: Vec3) -> Vec3 {
    let split = parent_world.to_scale_rotation_translation().0;
    let parent = glam::Mat3::from_mat4(parent_world);
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

/// Whether `m`, whose axis lengths are `size`, is a uniform scale times a rotation:
/// equal lengths and square axes, both to within [`UNIFORM`].
fn conformal(m: glam::Mat3, size: Vec3) -> bool {
    let top = size.max_element();
    let square = top * top;
    let (x, y, z) = (m.x_axis, m.y_axis, m.z_axis);
    top - size.min_element() <= UNIFORM * top
        && [x.dot(y), x.dot(z), y.dot(z)]
            .iter()
            .all(|d| d.abs() <= UNIFORM * square)
}

/// The reflection a pose carries: the sign of each axis of its signed world
/// scale ([`Pose::under`]), `±1` each, all `+1` for no mirror. The pose's world
/// matrix is its [`world_rotation`] times `mirror` times the scale's magnitude.
pub fn mirror(signed_scale: Vec3) -> Vec3 {
    Vec3::select(signed_scale.cmplt(Vec3::ZERO), -Vec3::ONE, Vec3::ONE)
}

/// A direction written in a pose's local space, as its world rotation must turn
/// it to land on the drawn world direction: `mirror · dir`, since the pose's
/// matrix draws a local `dir` along `R_w · mirror · dir` (#1125). An aim axis is
/// one. No mirror returns `dir` bit for bit.
pub fn mirrored_direction(mirror: Vec3, dir: Vec3) -> Vec3 {
    mirror * dir
}

/// A frame written in a pose's local space by its X and Y axes (a joint's `axis`
/// and `swing_axis`), as its world rotation must turn it: the frame seen through
/// the mirror as a rotation, the way [`world_rotation`] sees a child's (#1125).
/// Its X stays on the drawn line, but in the mirror image's sense: a turn about
/// `a` mirrors to a turn about `−mirror·a` when the mirror reflects (an axis of
/// rotation is a pseudovector), so a joint's limits act on the mirror-image
/// side. Y is mirrored as a direction, and the frame they make stays a proper
/// rotation. Its own inverse; no mirror returns both axes bit for bit.
///
/// Only the twist about X is the exact mirror image. Under a reflection, a turn
/// about the frame's Y or Z comes out in the opposite sense to the mirror
/// image's (the frame is `D·B·F`, not `−D·B`). That is harmless while swing
/// limits are symmetric, `±swing_limit` and `±swing2_limit`, which are the only
/// ones `physics::joints::joint_builder` sets. An asymmetric swing limit would
/// need the swing signs flipped too.
pub fn mirrored_frame_axes(mirror: Vec3, x: Vec3, y: Vec3) -> (Vec3, Vec3) {
    let sense = mirror.x * mirror.y * mirror.z;
    (sense * (mirror * x), mirror * y)
}

/// The rotation glam splits out of `parent_world`, or identity when the matrix
/// is degenerate: a zero scale axis has no direction, and glam divides by it,
/// giving a NaN rotation that would poison every pose and AABB below it (#1140).
fn parent_rotation(parent_world: Mat4) -> Quat {
    let (_, rot, _) = parent_world.to_scale_rotation_translation();
    if rot.is_finite() {
        rot
    } else {
        Quat::IDENTITY
    }
}

/// `rotation` seen through a mirroring parent's reflection, `F·R·F`: a rotation
/// about the axis reflected as a pseudovector, `(x, y, z, w) → (x, −y, −z, w)`.
/// Its own inverse, so one function serves both directions.
fn through_mirror(parent_world: Mat4, rotation: Quat) -> Quat {
    if parent_world.determinant() < 0.0 {
        Quat::from_xyzw(rotation.x, -rotation.y, -rotation.z, rotation.w)
    } else {
        rotation
    }
}

#[cfg(test)]
mod tests;
