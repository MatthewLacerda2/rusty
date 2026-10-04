//! src/physics/convert.rs — engine <-> rapier pose conversions.
//!
//! rapier/parry speak glam through `glamx`, the same glam the engine uses, so
//! rapier's `Vector` *is* `glam::Vec3` and its `Rotation` *is* `glam::Quat`: those
//! cross the boundary as-is. The one type that still differs is rapier's `Pose`
//! (a rigid transform), and the engine keeps its poses as a `(Vec3, Quat)` pair;
//! these two helpers are that crossing, kept in one place.

use glam::{Quat, Vec3};
use rapier3d::math::Pose;

/// Engine position + rotation -> rapier `Pose` (rigid-body pose).
pub fn to_pose(pos: Vec3, rot: Quat) -> Pose {
    Pose::from_parts(pos, rot)
}

/// rapier `Pose` -> engine (position, rotation).
pub fn from_pose(iso: &Pose) -> (Vec3, Quat) {
    (iso.translation, iso.rotation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn pose_round_trips_through_rapier() {
        // A non-identity pose so a conversion returning identity (or swapping
        // its parts) is caught: a 90° yaw at an offset must survive the
        // engine -> rapier -> engine round trip exactly.
        let (pos, rot) = (Vec3::new(1.0, -2.0, 3.5), Quat::from_rotation_y(FRAC_PI_2));
        let (p, q) = from_pose(&to_pose(pos, rot));
        assert_eq!(p, pos);
        assert_eq!(q, rot);
    }
}
