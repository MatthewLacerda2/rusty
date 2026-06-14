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
