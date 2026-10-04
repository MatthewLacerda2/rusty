//! src/physics/convert.rs — engine glam <-> rapier math boundary conversions.
//!
//! rapier/parry speak their own glam (`glamx`, a newer glam than the engine's):
//! `Vector`, `Rotation` and `Pose`. The two crates' `Vec3`/`Quat` are distinct
//! types, so every crossing goes through here and the rest of the physics module —
//! and the whole engine — keeps the engine's glam as its facing math.

use glam::{Quat, Vec3};
use rapier3d::math::{Pose, Rotation, Vector};

/// Engine `Vec3` -> rapier `Vector` (also rapier's point type).
pub fn to_rp_vec(v: Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}

/// rapier `Vector` (or point) -> engine `Vec3`.
pub fn from_rp_vec(v: Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Engine `Quat` -> rapier `Rotation`.
pub fn to_rp_quat(q: Quat) -> Rotation {
    Rotation::from_xyzw(q.x, q.y, q.z, q.w)
}

/// rapier `Rotation` -> engine `Quat`.
pub fn from_rp_quat(q: Rotation) -> Quat {
    Quat::from_xyzw(q.x, q.y, q.z, q.w)
}

/// Engine position + rotation -> rapier `Pose` (rigid-body pose).
pub fn to_pose(pos: Vec3, rot: Quat) -> Pose {
    Pose::from_parts(to_rp_vec(pos), to_rp_quat(rot))
}

/// rapier `Pose` -> engine (position, rotation).
pub fn from_pose(iso: &Pose) -> (Vec3, Quat) {
    (from_rp_vec(iso.translation), from_rp_quat(iso.rotation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn pose_round_trips_through_rapier() {
        // A non-identity pose so a conversion returning identity (or swapping
        // quaternion lanes) is caught: a 90° yaw at an offset must survive the
        // engine -> rapier -> engine round trip exactly.
        let (pos, rot) = (Vec3::new(1.0, -2.0, 3.5), Quat::from_rotation_y(FRAC_PI_2));
        let (p, q) = from_pose(&to_pose(pos, rot));
        assert_eq!(p, pos);
        assert_eq!(q, rot);
    }
}
