use glam::Vec3;
use serde::{Deserialize, Serialize};

const SAME_AT_M: f32 = 1e-3;
const SAME_YAW_DEG: f32 = 0.01;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ViewKey {
    pub label: String,
    pub camera: Option<u32>,
    pub at: [f32; 3],
    pub yaw: f32,
}

impl PartialEq for ViewKey {
    fn eq(&self, other: &Self) -> bool {
        let moved = Vec3::from(self.at).distance(Vec3::from(other.at));
        let turned = (self.yaw - other.yaw).rem_euclid(360.0);
        let turned = turned.min(360.0 - turned);
        self.label == other.label
            && self.camera == other.camera
            && moved <= SAME_AT_M
            && turned <= SAME_YAW_DEG
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(at: [f32; 3], yaw: f32) -> ViewKey {
        ViewKey {
            label: "nav 0 yaw 90".into(),
            camera: None,
            at,
            yaw,
        }
    }

    #[test]
    fn a_rounding_error_is_the_same_viewpoint_and_a_centimetre_is_another() {
        let base = key([10.0, 1.7, -4.0], 90.0);
        assert_eq!(base, key([10.00001, 1.7, -4.0], 90.0), "moved 1e-5 m");
        assert_eq!(base, key([10.0, 1.7, -4.0], 90.005), "turned 0.005°");
        assert_ne!(base, key([10.01, 1.7, -4.0], 90.0), "moved 1 cm");
        assert_ne!(base, key([10.0, 1.7, -4.0], 90.1), "turned 0.1°");
        assert_eq!(key([0.0; 3], 359.999), key([0.0; 3], -0.001), "one yaw");
        let other_camera = ViewKey {
            camera: Some(3),
            ..base.clone()
        };
        assert_ne!(base, other_camera);
    }
}
