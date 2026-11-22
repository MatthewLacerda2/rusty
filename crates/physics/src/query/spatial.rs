use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use super::is_live;
use crate::convert::to_pose;
use crate::shape::hull;
use crate::world::PhysicsWorld;

impl PhysicsWorld {
    pub fn overlap_sphere(
        &self,
        center: Vec3,
        radius: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<u32> {
        self.overlap(to_pose(center, Quat::IDENTITY), &Ball::new(radius), &accept)
    }

    pub fn overlap_box(
        &self,
        center: Vec3,
        half_extents: Vec3,
        rotation: Quat,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<u32> {
        let pose = to_pose(center, rotation);
        self.overlap(pose, &Cuboid::new(half_extents), &accept)
    }

    pub fn overlap_capsule(
        &self,
        a: Vec3,
        b: Vec3,
        radius: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<u32> {
        let capsule = Capsule::new(a, b, radius);
        self.overlap(to_pose(Vec3::ZERO, Quat::IDENTITY), &capsule, &accept)
    }

    /// Sorted ascending: broad-phase traversal order must not reach scripts.
    fn overlap(&self, pose: Pose, shape: &dyn Shape, accept: &dyn Fn(u32) -> bool) -> Vec<u32> {
        let predicate = self.handle_accepts(accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let mut ids: Vec<u32> = self
            .queries(filter)
            .intersect_shape(pose, shape)
            .filter_map(|(handle, _)| self.collider_to_id.get(&handle).copied())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// A point inside is its own closest point (Unity `Collider.ClosestPoint`).
    pub fn closest_point_on(&self, id: u32, point: Vec3) -> Option<Vec3> {
        let collider = self.collider_of(id)?;
        if hull_holds(collider, point) == Some(true) {
            return Some(point);
        }
        let projection = collider
            .shape()
            .project_point(collider.position(), point, true);
        Some(projection.point)
    }

    pub fn collider_contains_point(&self, id: u32, point: Vec3) -> Option<bool> {
        let collider = self.collider_of(id)?;
        Some(
            hull_holds(collider, point)
                .unwrap_or_else(|| collider.shape().contains_point(collider.position(), point)),
        )
    }

    fn collider_of(&self, id: u32) -> Option<&Collider> {
        let collider = self.colliders.get(*self.id_to_collider.get(&id)?)?;
        is_live(&self.bodies, collider).then_some(collider)
    }
}

/// A hull with volume answers by its face planes, since parry's projection can call a
/// point on its corner far outside; `None` for any other shape, a flat hull included.
fn hull_holds(collider: &Collider, point: Vec3) -> Option<bool> {
    let hull = (collider.shape().as_convex_polyhedron()).filter(|h| hull::has_volume(h))?;
    Some(hull::holds(hull, collider.position(), point))
}
