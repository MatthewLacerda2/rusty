use glam::{Mat4, Vec3};

use super::Decal;
use crate::Scene;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalPose {
    /// Object→world matrix mapping the unit cube `[-0.5, 0.5]³` to the box.
    pub model: Mat4,
    pub opacity: f32,
}

impl DecalPose {
    pub fn unowned(decal: &Decal) -> Self {
        Self {
            model: decal.model_matrix(),
            opacity: decal.opacity(),
        }
    }

    pub fn centre(&self) -> Vec3 {
        self.model.w_axis.truncate()
    }

    pub fn radius(&self) -> f32 {
        let (x, y, z) = (
            self.model.x_axis.truncate(),
            self.model.y_axis.truncate(),
            self.model.z_axis.truncate(),
        );
        [x + y + z, x + y - z, x - y + z, -x + y + z]
            .into_iter()
            .map(Vec3::length)
            .fold(0.0, f32::max)
            * 0.5
    }
}

impl Scene {
    pub fn decal_pose(&self, decal: &Decal) -> Option<DecalPose> {
        let mut pose = DecalPose::unowned(decal);
        if let Some(owner) = decal.owner {
            if !self.world.is_active_in_hierarchy(owner.entity) {
                return None;
            }
            pose.model = self.world.local_to_world(owner.entity) * owner.local;
        }
        (pose.opacity > 0.0).then_some(pose)
    }
}
