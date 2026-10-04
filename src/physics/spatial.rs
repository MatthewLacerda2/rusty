//! src/physics/spatial.rs — area-shaped queries over the rapier world (#311).
//!
//! `query` holds the line-shaped ray casts; this file holds everything with a
//! volume or a specific collider in mind: overlaps ("what colliders are inside
//! this sphere/box/capsule right now?"), the sphere-cast (a raycast with
//! thickness), and the per-collider point queries (closest point, containment).
//! Every query routes through the same live query view the hitscan uses, so
//! a script's area query and the engine's agree about the same world.

use glam::{Quat, Vec3};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::prelude::*;

use super::convert::to_pose;
use super::query::{is_live, RayHit};
use super::world::PhysicsWorld;

impl PhysicsWorld {
    /// Entity ids whose colliders intersect the sphere at `center` — Unity's
    /// `Physics.OverlapSphere`. Only entities accepted by `accept` are reported.
    pub fn overlap_sphere(
        &self,
        center: Vec3,
        radius: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<u32> {
        self.overlap(center, &Ball::new(radius), &accept)
    }

    /// Like [`Self::overlap_sphere`] with an axis-aligned box of `half_extents`
    /// — Unity's `Physics.OverlapBox` (identity orientation).
    pub fn overlap_box(
        &self,
        center: Vec3,
        half_extents: Vec3,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<u32> {
        self.overlap(center, &Cuboid::new(half_extents), &accept)
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
        self.overlap(Vec3::ZERO, &capsule, &accept)
    }

    /// Shared overlap walk: every accepted collider intersecting `shape` placed
    /// at `center`, as entity ids sorted ascending — the broad-phase traversal
    /// order is an implementation detail, so the result is normalized to keep
    /// script-visible output deterministic.
    fn overlap(&self, center: Vec3, shape: &dyn Shape, accept: &dyn Fn(u32) -> bool) -> Vec<u32> {
        let predicate = self.handle_accepts(accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let mut ids: Vec<u32> = self
            .queries(filter)
            .intersect_shape(to_pose(center, Quat::IDENTITY), shape)
            .filter_map(|(handle, _)| self.collider_to_id.get(&handle).copied())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// [`Self::sphere_cast_hit`] reduced to (entity id, distance traveled).
    pub fn cast_sphere_filtered(
        &self,
        origin: Vec3,
        dir: Vec3,
        radius: f32,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<(u32, f32)> {
        self.sphere_cast_hit(origin, dir, radius, max_toi, accept)
            .map(|hit| (hit.id, hit.distance))
    }

    /// Closest accepted collider touched by a sphere of `radius` swept from
    /// `origin` along `dir` — Unity's `Physics.SphereCast`, i.e. a thick
    /// raycast, the volume sibling of [`Self::raycast_hit`]. `distance` is how
    /// far the sphere's center traveled; `point` is the contact on the struck
    /// surface and `normal` that surface's outward normal, both world-space.
    pub fn sphere_cast_hit(
        &self,
        origin: Vec3,
        dir: Vec3,
        radius: f32,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<RayHit> {
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let (handle, hit) = self.queries(filter).cast_shape(
            &to_pose(origin, Quat::IDENTITY),
            dir.normalize(),
            &Ball::new(radius),
            ShapeCastOptions::with_max_time_of_impact(max_toi),
        )?;
        // rapier's normal1 is the struck surface's, in world space. Its witness
        // point carries GJK slop; for a sphere the contact is exactly one
        // radius from the impact-time center, against the normal.
        let normal = hit.normal1;
        let center = origin + dir.normalize() * hit.time_of_impact;
        Some(RayHit {
            id: *self.collider_to_id.get(&handle)?,
            distance: hit.time_of_impact,
            point: center - normal * radius,
            normal,
        })
    }

    /// Nearest point on `id`'s live collider to `point`, solid — a point inside
    /// the collider is its own closest point (Unity `Collider.ClosestPoint`).
    /// `None` when the entity has no collider in the live world.
    pub fn closest_point_on(&self, id: u32, point: Vec3) -> Option<Vec3> {
        let collider = self.collider_of(id)?;
        let projection = collider
            .shape()
            .project_point(collider.position(), point, true);
        Some(projection.point)
    }

    /// Whether `point` lies inside `id`'s live collider. `None` when the entity
    /// has no collider in the live world.
    pub fn collider_contains_point(&self, id: u32, point: Vec3) -> Option<bool> {
        let collider = self.collider_of(id)?;
        Some(collider.shape().contains_point(collider.position(), point))
    }

    /// The live collider built for entity `id` (the entity that owns the
    /// collider, even when it is a compound part of an ancestor's body), if any.
    /// A collider that is not [`is_live`] (deactivated entity) counts as absent.
    fn collider_of(&self, id: u32) -> Option<&Collider> {
        let collider = self.colliders.get(*self.id_to_collider.get(&id)?)?;
        is_live(&self.bodies, collider).then_some(collider)
    }
}
