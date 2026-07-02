//! src/physics/spatial.rs — area-shaped queries over the rapier world (#311).
//!
//! `query` holds the line-shaped ray casts; this file holds everything with a
//! volume or a specific collider in mind: overlaps ("what colliders are inside
//! this sphere/box/capsule right now?"), the sphere-cast (a raycast with
//! thickness), and the per-collider point queries (closest point, containment).
//! Every query routes through the same live query pipeline the hitscan uses, so
//! a script's area query and the engine's agree about the same world.

use glam::{Quat, Vec3};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::prelude::*;

use super::convert::{from_na_point, to_iso, to_na_vec};
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
        self.overlap(center, &Cuboid::new(to_na_vec(half_extents)), &accept)
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
        let capsule = Capsule::new(to_na_vec(a).into(), to_na_vec(b).into(), radius);
        self.overlap(Vec3::ZERO, &capsule, &accept)
    }

    /// Shared overlap walk: every accepted collider intersecting `shape` placed
    /// at `center`, as entity ids sorted ascending — the broad-phase traversal
    /// order is an implementation detail, so the result is normalized to keep
    /// script-visible output deterministic.
    fn overlap(&self, center: Vec3, shape: &dyn Shape, accept: &dyn Fn(u32) -> bool) -> Vec<u32> {
        let predicate = self.handle_accepts(accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let mut ids = Vec::new();
        self.query_pipeline.intersections_with_shape(
            &self.bodies,
            &self.colliders,
            &to_iso(center, Quat::IDENTITY),
            shape,
            filter,
            |handle| {
                if let Some(&id) = self.collider_to_id.get(&handle) {
                    ids.push(id);
                }
                true
            },
        );
        ids.sort_unstable();
        ids
    }

    /// Closest accepted collider touched by a sphere of `radius` swept from
    /// `origin` along `dir` — Unity's `Physics.SphereCast`, i.e. a thick
    /// raycast. Returns (entity id, distance traveled along the sweep), the
    /// volume sibling of [`Self::cast_ray_filtered`].
    pub fn cast_sphere_filtered(
        &self,
        origin: Vec3,
        dir: Vec3,
        radius: f32,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<(u32, f32)> {
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        self.query_pipeline
            .cast_shape(
                &self.bodies,
                &self.colliders,
                &to_iso(origin, Quat::IDENTITY),
                &to_na_vec(dir.normalize()),
                &Ball::new(radius),
                ShapeCastOptions::with_max_time_of_impact(max_toi),
                filter,
            )
            .and_then(|(handle, hit)| {
                self.collider_to_id
                    .get(&handle)
                    .map(|&id| (id, hit.time_of_impact))
            })
    }

    /// Nearest point on `id`'s live collider to `point`, solid — a point inside
    /// the collider is its own closest point (Unity `Collider.ClosestPoint`).
    /// `None` when the entity has no collider in the live world.
    pub fn closest_point_on(&self, id: u32, point: Vec3) -> Option<Vec3> {
        let collider = self.collider_of(id)?;
        let projection =
            collider
                .shape()
                .project_point(collider.position(), &to_na_vec(point).into(), true);
        Some(from_na_point(projection.point))
    }

    /// Whether `point` lies inside `id`'s live collider. `None` when the entity
    /// has no collider in the live world.
    pub fn collider_contains_point(&self, id: u32, point: Vec3) -> Option<bool> {
        let collider = self.collider_of(id)?;
        Some(
            collider
                .shape()
                .contains_point(collider.position(), &to_na_vec(point).into()),
        )
    }

    /// Adapt an entity-id acceptance test to rapier's collider-handle predicate.
    fn handle_accepts<'a>(
        &'a self,
        accept: &'a dyn Fn(u32) -> bool,
    ) -> impl Fn(ColliderHandle, &Collider) -> bool + 'a {
        move |handle, _| {
            self.collider_to_id
                .get(&handle)
                .is_some_and(|&id| accept(id))
        }
    }

    /// The live collider built for entity `id`, if any. A linear scan of the
    /// one-collider-per-entity handle map — fine at query rates, and it avoids
    /// storing a second (inverse) map on the world.
    fn collider_of(&self, id: u32) -> Option<&Collider> {
        let (&handle, _) = self.collider_to_id.iter().find(|&(_, &i)| i == id)?;
        self.colliders.get(handle)
    }
}
