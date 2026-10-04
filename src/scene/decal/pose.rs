//! Where a decal is this frame, and how much of it is left (#639): the box an owned
//! decal resolves to through its owner's world matrix, and its fade.

use glam::{Mat4, Vec3};

use super::Decal;
use crate::scene::Scene;

/// One decal as this frame draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalPose {
    /// Object→world matrix mapping the unit cube `[-0.5, 0.5]³` to the box.
    pub model: Mat4,
    /// `1` whole to `0` gone; scales the decal's coverage, so every blend weight.
    pub opacity: f32,
}

impl DecalPose {
    /// The pose of a decal that sticks to nothing: its spawn box.
    pub fn unowned(decal: &Decal) -> Self {
        Self {
            model: decal.model_matrix(),
            opacity: decal.opacity(),
        }
    }

    /// The box's world-space centre.
    pub fn centre(&self) -> Vec3 {
        self.model.w_axis.truncate()
    }

    /// The radius of the sphere around [`Self::centre`] that holds the whole box,
    /// sheared or not: half its longest diagonal.
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
    /// Where `decal` draws this frame, or `None` when it draws nothing: its owner is
    /// inactive or gone, or it has faded out. An owned box is its owner's world
    /// matrix (the per-frame store, #331) times the box in the owner's space.
    pub fn decal_pose(&self, decal: &Decal) -> Option<DecalPose> {
        let mut pose = DecalPose::unowned(decal);
        if let Some(owner) = decal.owner {
            if !self.world.is_active(owner.entity) {
                return None;
            }
            pose.model = self.world_matrix(owner.entity) * owner.local;
        }
        (pose.opacity > 0.0).then_some(pose)
    }
}
