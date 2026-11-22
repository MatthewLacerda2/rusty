use glam::{Quat, Vec3};
use rapier3d::math::Pose;

pub fn to_pose(pos: Vec3, rot: Quat) -> Pose {
    Pose::from_parts(pos, rot)
}

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
