//! World space has one store: a dirty-flagged matrix cache that a read cleans parents first, so it
//! is never stale. A write is O(1): it dirties the subtree and marks its colliders' bounds due a
//! refresh. A mirror stays in the scale; under a singular parent a world setter keeps the local
//! value it cannot solve for, as Unity does.

mod bounds;
mod collider;
#[cfg(test)]
mod queue_tests;
mod store;
#[cfg(test)]
mod tests;
mod write;

use glam::{Mat4, Quat, Vec3};

use crate::World;
use rusty_components::particle::EmitterFrame;
use rusty_components::transform::{
    local_rotation, mirror, mirrored_direction, mirrored_frame_axes,
};
use rusty_components::Pose;

pub(crate) use bounds::BoundsQueue;
pub use collider::ColliderMut;
pub(crate) use store::WorldStore;
pub use write::TransformMut;

impl World {
    pub fn local_to_world(&self, id: u32) -> Mat4 {
        self.world_space.resolve(self, id)
    }

    fn parent_to_world(&self, id: u32) -> Option<Mat4> {
        self.ancestors(id).next().map(|p| self.local_to_world(p))
    }

    pub fn world_pose(&self, id: u32) -> Option<Pose> {
        let local = self.transform(id)?.pose();
        Some(match self.parent_to_world(id) {
            Some(parent) => local.under(parent),
            None => local,
        })
    }

    /// Unity's default *Local* Scaling Mode: own scale only, every parent's ignored.
    pub fn emitter_frame(&self, id: u32) -> EmitterFrame {
        let Some(scale) = self.transform(id).map(|t| t.scale) else {
            return EmitterFrame::IDENTITY;
        };
        EmitterFrame::new(self.world_position(id), self.world_rotation(id), scale)
    }

    pub fn world_position(&self, id: u32) -> Vec3 {
        self.local_to_world(id).w_axis.truncate()
    }

    pub fn world_rotation(&self, id: u32) -> Quat {
        self.world_pose(id).map_or(Quat::IDENTITY, |p| p.rot)
    }

    pub fn lossy_scale(&self, id: u32) -> Vec3 {
        self.world_pose(id).map_or(Vec3::ONE, |p| p.scale)
    }

    /// A test hook (`tests/proptest/world_space.rs`).
    pub fn is_mirrored(&self, id: u32) -> bool {
        self.local_to_world(id).determinant() < 0.0
    }

    pub fn transform_point(&self, id: u32, point: Vec3) -> Vec3 {
        self.local_to_world(id).transform_point3(point)
    }

    pub fn transform_direction(&self, id: u32, direction: Vec3) -> Vec3 {
        let Some(pose) = self.world_pose(id) else {
            return direction;
        };
        pose.rot * mirrored_direction(mirror(pose.scale), direction)
    }

    pub fn inverse_transform_frame(&self, id: u32, x: Vec3, y: Vec3) -> (Vec3, Vec3) {
        let Some(pose) = self.world_pose(id) else {
            return (x, y);
        };
        let inv = pose.rot.inverse();
        mirrored_frame_axes(mirror(pose.scale), inv * x, inv * y)
    }

    pub fn inverse_transform_point(&self, id: u32, point: Vec3) -> Option<Vec3> {
        invert(self.local_to_world(id)).map(|inv| inv.transform_point3(point))
    }

    pub fn local_position_for(&self, id: u32, position: Vec3) -> Vec3 {
        let kept = || self.transform(id).map_or(Vec3::ZERO, |t| t.position);
        match self.parent_to_world(id) {
            None => position,
            Some(parent) => invert(parent).map_or_else(kept, |inv| inv.transform_point3(position)),
        }
    }

    pub fn local_rotation_for(&self, id: u32, rotation: Quat) -> Quat {
        match self.parent_to_world(id) {
            None => rotation,
            Some(parent) => local_rotation(parent, rotation),
        }
    }

    pub fn set_world_position(&mut self, id: u32, position: Vec3) -> bool {
        let local = self.local_position_for(id, position);
        self.transform_mut(id)
            .map(|mut t| t.position = local)
            .is_some()
    }

    pub fn set_world_rotation(&mut self, id: u32, rotation: Quat) -> bool {
        let local = self.local_rotation_for(id, rotation);
        self.transform_mut(id)
            .map(|mut t| t.rotation = local)
            .is_some()
    }
}

fn invert(m: Mat4) -> Option<Mat4> {
    let inverse = m.inverse();
    (m.determinant() != 0.0 && inverse.is_finite()).then_some(inverse)
}
