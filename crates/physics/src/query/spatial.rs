//! crates/physics/src/query/spatial.rs — area-shaped queries over the rapier world (#311).
//!
//! `query` holds the line-shaped ray casts; this file holds everything with a
//! volume or a specific collider in mind: overlaps ("what colliders are inside
//! this sphere/box/capsule right now?") and the per-collider point queries
//! (closest point, containment). The shape casts live in `sweep`.
//! Every query routes through the same live query view the hitscan uses, so
//! a script's area query and the engine's agree about the same world.

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use super::is_live;
use crate::convert::to_pose;
use crate::shape::hull;
use crate::world::PhysicsWorld;

impl PhysicsWorld {
    /// Entity ids whose colliders intersect the sphere at `center` — Unity's
    /// `Physics.OverlapSphere`. Only entities accepted by `accept` are reported.
    pub fn overlap_sphere(
        &self,
        center: Vec3,
        radius: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<u32> {
        self.overlap(to_pose(center, Quat::IDENTITY), &Ball::new(radius), &accept)
    }

    /// Like [`Self::overlap_sphere`] with a box of `half_extents` turned by
    /// `rotation` — Unity's `Physics.OverlapBox` and its `orientation`.
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

    /// Like [`Self::overlap_sphere`] with a capsule spanning `a`→`b` — Unity's
    /// `Physics.OverlapCapsule` (the two sphere centers plus the radius).
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

    /// Shared overlap walk: every accepted collider intersecting `shape` placed
    /// at `pose`, as entity ids sorted ascending — the broad-phase traversal
    /// order is an implementation detail, so the result is normalized to keep
    /// script-visible output deterministic.
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

    /// Nearest point on `id`'s live collider to `point`, solid — a point inside
    /// the collider is its own closest point (Unity `Collider.ClosestPoint`).
    /// `None` when the entity has no collider in the live world. A convex hull
    /// answers an inside point by its face planes (#1211).
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

    /// Whether `point` lies inside `id`'s live collider. `None` when the entity
    /// has no collider in the live world. A convex hull answers by its face
    /// planes (#1211).
    pub fn collider_contains_point(&self, id: u32, point: Vec3) -> Option<bool> {
        let collider = self.collider_of(id)?;
        Some(
            hull_holds(collider, point)
                .unwrap_or_else(|| collider.shape().contains_point(collider.position(), point)),
        )
    }

    /// The live collider built for entity `id` (the entity that owns the
    /// collider, even when it is a compound part of an ancestor's body), if any.
    /// A collider that is not [`is_live`] (deactivated entity) counts as absent.
    fn collider_of(&self, id: u32) -> Option<&Collider> {
        let collider = self.colliders.get(*self.id_to_collider.get(&id)?)?;
        is_live(&self.bodies, collider).then_some(collider)
    }
}

/// [`hull::holds`] for a convex mesh collider with volume; `None` for any other
/// shape, a flat hull included. parry's point projection can misreport a point on
/// a hull's corner as far outside it.
fn hull_holds(collider: &Collider, point: Vec3) -> Option<bool> {
    let hull = (collider.shape().as_convex_polyhedron()).filter(|h| hull::has_volume(h))?;
    Some(hull::holds(hull, collider.position(), point))
}
