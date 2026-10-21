//! src/ecs/world_space/ — where an entity is in the world, and the one store of it (#1170).
//!
//! Unity's model: a Transform owns both its local view and its world view, and nothing
//! else derives world space. A root entity's local Transform *is* its world transform;
//! everything under a parent reads its world view from here:
//! - [`World::local_to_world`], the world matrix, kept in a dirty-flagged store
//!   (`store.rs`) that a read cleans, parents first, so it is never stale, mid-tick
//!   included;
//! - [`World::world_pose`], the split physics poses a body with: [`World::world_position`],
//!   [`World::world_rotation`] (a mirror stays in the scale, #1119) and
//!   [`World::lossy_scale`], the world matrix's column lengths along the entity's own
//!   axes, signed by the mirror (#1123, #1128, #1156);
//! - [`World::transform_point`], [`World::transform_direction`] (through the entity's
//!   own mirror, #1145), [`World::transform_vector`] and
//!   [`World::inverse_transform_point`];
//! - [`World::set_world_position`] and [`World::set_world_rotation`], which under a
//!   singular parent keep the local value they cannot solve for, as Unity does (#1164).
//!
//! A write is a [`TransformMut`] landing (`write.rs`), O(1): its subtree goes dirty before
//! the next read, and its colliders' AABBs are due a refresh (`bounds.rs`), which
//! [`World::collider_bounds`] never reads stale (#1157). Re-parenting and a despawn queue
//! what they re-point the same way; a document insert invalidates every slot.

mod bounds;
mod collider;
#[cfg(test)]
mod queue_tests;
mod store;
#[cfg(test)]
mod tests;
mod write;

use glam::{Mat4, Quat, Vec3};

use crate::components::transform::{local_rotation, mirror, mirrored_direction};
use crate::components::Pose;
use crate::ecs::World;

pub(in crate::ecs) use bounds::BoundsQueue;
pub use collider::ColliderMut;
pub(in crate::ecs) use store::WorldStore;
pub use write::TransformMut;

impl World {
    /// `id`'s world matrix: its local Transform under every ancestor's. Identity for a
    /// dead id. A malformed parent cycle ends the climb (#991).
    pub fn local_to_world(&self, id: u32) -> Mat4 {
        self.world_space.resolve(self, id)
    }

    /// The world matrix `id`'s local Transform sits under: its live parent's, or `None`
    /// for a root.
    fn parent_to_world(&self, id: u32) -> Option<Mat4> {
        self.ancestors(id).next().map(|p| self.local_to_world(p))
    }

    /// `id`'s world [`Pose`]: its local Transform [`Pose::under`] its parent's world
    /// matrix, or the local pose as-is for a root, bit for bit. The pose physics builds a
    /// body at and a collider's world AABB reads (#1140). `None` for a dead id.
    pub fn world_pose(&self, id: u32) -> Option<Pose> {
        let local = self.transform(id)?.pose();
        Some(match self.parent_to_world(id) {
            Some(parent) => local.under(parent),
            None => local,
        })
    }

    /// Where `id` stands in the world (the origin for a dead id).
    pub fn world_position(&self, id: u32) -> Vec3 {
        self.local_to_world(id).w_axis.truncate()
    }

    /// `id`'s world rotation, a proper rotation whatever mirror the hierarchy carries
    /// (identity for a dead id).
    pub fn world_rotation(&self, id: u32) -> Quat {
        self.world_pose(id).map_or(Quat::IDENTITY, |p| p.rot)
    }

    /// Unity's `lossyScale`: the world matrix's scale along `id`'s own axes, signed per
    /// the mirror convention ([`Pose::under`]). Exact unless the hierarchy shears, which
    /// no TRS can represent. `ONE` for a dead id.
    pub fn lossy_scale(&self, id: u32) -> Vec3 {
        self.world_pose(id).map_or(Vec3::ONE, |p| p.scale)
    }

    /// The reflection `id`'s world pose carries, `±1` per axis: the signs of its
    /// [`World::lossy_scale`].
    pub fn world_mirror(&self, id: u32) -> Vec3 {
        mirror(self.lossy_scale(id))
    }

    /// Whether `id`'s world matrix reflects (its determinant is negative): what the
    /// renderer flips winding for.
    pub fn is_mirrored(&self, id: u32) -> bool {
        self.local_to_world(id).determinant() < 0.0
    }

    /// A point in `id`'s local space, in world space.
    pub fn transform_point(&self, id: u32, point: Vec3) -> Vec3 {
        self.local_to_world(id).transform_point3(point)
    }

    /// A vector in `id`'s local space, in world space: scaled and turned, not moved.
    pub fn transform_vector(&self, id: u32, vector: Vec3) -> Vec3 {
        self.local_to_world(id).transform_vector3(vector)
    }

    /// A direction in `id`'s local space, in world space: turned, not scaled, and through
    /// the entity's mirror, so it lands on the drawn direction even when `id`'s own scale
    /// is negative (#1125, #1145). Keeps `direction`'s length.
    pub fn transform_direction(&self, id: u32, direction: Vec3) -> Vec3 {
        let Some(pose) = self.world_pose(id) else {
            return direction;
        };
        pose.rot * mirrored_direction(mirror(pose.scale), direction)
    }

    /// A world point in `id`'s local space. `None` when `id`'s world matrix is singular
    /// (a zero scale on it or above it), which has no inverse.
    pub fn inverse_transform_point(&self, id: u32, point: Vec3) -> Option<Vec3> {
        invert(self.local_to_world(id)).map(|inv| inv.transform_point3(point))
    }

    /// The local position that puts `id` at `position` in world space. Under a singular
    /// parent it is the local position `id` already has, as Unity keeps it: an axis the
    /// parent collapsed cannot be solved for, and a NaN would not survive a save (#1164).
    /// The origin for a dead id.
    pub fn local_position_for(&self, id: u32, position: Vec3) -> Vec3 {
        let kept = || self.transform(id).map_or(Vec3::ZERO, |t| t.position);
        match self.parent_to_world(id) {
            None => position,
            Some(parent) => invert(parent).map_or_else(kept, |inv| inv.transform_point3(position)),
        }
    }

    /// The local rotation that gives `id` the world rotation `rotation`: the inverse of
    /// [`World::world_rotation`], a mirroring parent included (#1119).
    pub fn local_rotation_for(&self, id: u32, rotation: Quat) -> Quat {
        match self.parent_to_world(id) {
            None => rotation,
            Some(parent) => local_rotation(parent, rotation),
        }
    }

    /// Move `id` to `position` in world space ([`World::local_position_for`]). `false`
    /// for a dead id.
    pub fn set_world_position(&mut self, id: u32, position: Vec3) -> bool {
        let local = self.local_position_for(id, position);
        self.transform_mut(id)
            .map(|mut t| t.position = local)
            .is_some()
    }

    /// Turn `id` to `rotation` in world space ([`World::local_rotation_for`]). `false`
    /// for a dead id.
    pub fn set_world_rotation(&mut self, id: u32, rotation: Quat) -> bool {
        let local = self.local_rotation_for(id, rotation);
        self.transform_mut(id)
            .map(|mut t| t.rotation = local)
            .is_some()
    }
}

/// `m`'s inverse, or `None` when it is singular: glam divides by the determinant, so a
/// collapsed axis gives infinities and NaNs.
fn invert(m: Mat4) -> Option<Mat4> {
    let inverse = m.inverse();
    (m.determinant() != 0.0 && inverse.is_finite()).then_some(inverse)
}
