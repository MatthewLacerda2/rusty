//! src/physics/convert.rs — glam <-> nalgebra boundary conversions.
//!
//! rapier/parry speak nalgebra (`Vector3`, `Isometry3`, `UnitQuaternion`); the
//! engine speaks glam (`Vec3`, `Quat`). All conversion lives here so the rest of
//! the physics module — and the whole engine — keeps glam as its facing math.

use glam::{Quat, Vec3};
use rapier3d::na::{self, Isometry3, UnitQuaternion, Vector3};

/// glam `Vec3` -> nalgebra `Vector3<f32>`.
pub fn to_na_vec(v: Vec3) -> Vector3<f32> {
    Vector3::new(v.x, v.y, v.z)
}

/// nalgebra `Vector3<f32>` -> glam `Vec3`.
pub fn from_na_vec(v: Vector3<f32>) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// nalgebra translation point -> glam `Vec3`.
pub fn from_na_point(p: na::Point3<f32>) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

/// glam `Quat` -> nalgebra `UnitQuaternion<f32>`.
pub fn to_na_quat(q: Quat) -> UnitQuaternion<f32> {
    UnitQuaternion::from_quaternion(na::Quaternion::new(q.w, q.x, q.y, q.z))
}

/// nalgebra `UnitQuaternion<f32>` -> glam `Quat`.
pub fn from_na_quat(q: &UnitQuaternion<f32>) -> Quat {
    Quat::from_xyzw(q.i, q.j, q.k, q.w)
}

/// glam position + rotation -> rapier `Isometry3<f32>` (rigid-body pose).
pub fn to_iso(pos: Vec3, rot: Quat) -> Isometry3<f32> {
    Isometry3::from_parts(to_na_vec(pos).into(), to_na_quat(rot))
}

/// rapier `Isometry3<f32>` -> glam (position, rotation).
pub fn from_iso(iso: &Isometry3<f32>) -> (Vec3, Quat) {
    (
        from_na_point(iso.translation.vector.into()),
        from_na_quat(&iso.rotation),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn quat_round_trips_through_nalgebra() {
        // A non-identity rotation so `from_na_quat` returning identity (or any fixed
        // default) is caught: a 90° yaw must survive the glam->na->glam round trip.
        let q = Quat::from_rotation_y(FRAC_PI_2);
        let back = from_na_quat(&to_na_quat(q));
        // Quaternion double cover: q and -q are the same rotation.
        assert!(
            back.abs_diff_eq(q, 1e-5) || back.abs_diff_eq(-q, 1e-5),
            "round-tripped quat {back:?} should match {q:?}"
        );
    }
}
