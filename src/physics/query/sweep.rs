//! src/physics/query/sweep.rs — shape casts (sweeps) over the rapier world
//! (#311, #1881).
//!
//! A [`Sweep`] is a sphere, capsule or oriented box placed in the world; it is
//! swept along a direction and reports the colliders it would strike: the
//! first one ([`PhysicsWorld::shape_cast_hit`], Unity's `SphereCast` /
//! `CapsuleCast` / `BoxCast`) or every one ([`PhysicsWorld::shape_cast_all`],
//! Unity's `SphereCastAll`). Both go through the same live query view, liveness
//! check and acceptance test as the ray casts, so every cast agrees about one
//! world.
//!
//! **A sweep that starts overlapping a collider hits it at distance 0**, in the
//! single and the multi-hit form alike (parry's `stop_at_penetration`), with the
//! penetration normal parry computes; Unity's single casts skip such a
//! collider instead, and its `*CastAll` reports it with a zeroed point.

use glam::{Quat, Vec3};
use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::query::{Ray, RayCast, ShapeCastHit, ShapeCastOptions};
use rapier3d::prelude::*;

use super::{sort_hits, RayHit};
use crate::physics::convert::to_pose;
use crate::physics::world::PhysicsWorld;

/// A volume to sweep, placed at its start: what a shape cast moves along its
/// direction. Sizes are trusted here (the Lua layer rejects a negative or
/// non-finite one).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sweep {
    /// A sphere at `center` (Unity's `SphereCast` origin and radius).
    Sphere { center: Vec3, radius: f32 },
    /// A capsule whose two sphere centers are `a` and `b` (Unity's
    /// `CapsuleCast` point1 / point2 / radius).
    Capsule { a: Vec3, b: Vec3, radius: f32 },
    /// A box at `center` with `half_extents`, turned by `rotation` (Unity's
    /// `BoxCast` center / halfExtents / orientation).
    Box {
        center: Vec3,
        half_extents: Vec3,
        rotation: Quat,
    },
}

impl Sweep {
    /// Run `f` over this sweep's start pose and parry shape.
    fn with_shape<R>(&self, f: impl FnOnce(&Pose, &dyn Shape) -> R) -> R {
        match *self {
            Sweep::Sphere { center, radius } => {
                f(&to_pose(center, Quat::IDENTITY), &Ball::new(radius))
            }
            Sweep::Capsule { a, b, radius } => f(
                &to_pose(Vec3::ZERO, Quat::IDENTITY),
                &Capsule::new(a, b, radius),
            ),
            Sweep::Box {
                center,
                half_extents,
                rotation,
            } => f(&to_pose(center, rotation), &Cuboid::new(half_extents)),
        }
    }

    /// The world-space contact of `hit`, the sweep having moved along the unit
    /// `dir`. A sphere's is exact: one radius from its impact-time center,
    /// against the normal (parry's witness point carries GJK slop). The other
    /// shapes take the witness on the struck collider.
    fn contact(&self, hit: &ShapeCastHit, dir: Vec3) -> Vec3 {
        match *self {
            Sweep::Sphere { center, radius } => {
                center + dir * hit.time_of_impact - hit.normal1 * radius
            }
            _ => hit.witness1,
        }
    }
}

impl PhysicsWorld {
    /// Closest accepted collider touched by a sphere of `radius` swept from
    /// `origin` along `dir`: [`Self::shape_cast_hit`] with a [`Sweep::Sphere`].
    pub fn sphere_cast_hit(
        &self,
        origin: Vec3,
        dir: Vec3,
        radius: f32,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<RayHit> {
        let sweep = Sweep::Sphere {
            center: origin,
            radius,
        };
        self.shape_cast_hit(sweep, dir, max_toi, accept)
    }

    /// The first accepted collider `sweep` strikes moving along `dir` within
    /// `max_toi`. `distance` is how far the shape traveled before impact;
    /// `point` is the contact and `normal` the struck surface's outward
    /// normal, both world-space.
    pub fn shape_cast_hit(
        &self,
        sweep: Sweep,
        dir: Vec3,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<RayHit> {
        let dir = dir.normalize();
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let options = ShapeCastOptions::with_max_time_of_impact(max_toi);
        let (handle, hit) = sweep
            .with_shape(|pose, shape| self.queries(filter).cast_shape(pose, dir, shape, options))?;
        self.sweep_hit(sweep, dir, handle, &hit)
    }

    /// Every accepted collider `sweep` strikes moving along `dir` within
    /// `max_toi`, each at its own first contact, nearest first (ties by entity
    /// id): Unity's `SphereCastAll` and kin. The candidates are the colliders
    /// whose query-tree boxes the shape's box meets along the way
    /// (`swept_candidates`); each is then cast exactly.
    pub fn shape_cast_all(
        &self,
        sweep: Sweep,
        dir: Vec3,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<RayHit> {
        let dir = dir.normalize();
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let queries = self.queries(filter);
        let options = ShapeCastOptions::with_max_time_of_impact(max_toi);
        let mut hits: Vec<RayHit> = sweep.with_shape(|pose, shape| {
            swept_candidates(&queries, shape.compute_aabb(pose), dir, max_toi)
                .filter_map(|(handle, collider)| {
                    let at = collider.position();
                    let hit = queries
                        .dispatcher
                        .cast_shapes(
                            &at.inv_mul(pose),
                            at.rotation.inverse() * dir,
                            collider.shape(),
                            shape,
                            options,
                        )
                        .ok()??
                        .transform1_by(at);
                    self.sweep_hit(sweep, dir, handle, &hit)
                })
                .collect()
        });
        sort_hits(&mut hits);
        hits
    }

    /// Resolve a world-space shape-cast hit to a [`RayHit`]; `None` when the
    /// collider belongs to no entity.
    fn sweep_hit(
        &self,
        sweep: Sweep,
        dir: Vec3,
        handle: ColliderHandle,
        hit: &ShapeCastHit,
    ) -> Option<RayHit> {
        Some(RayHit {
            id: *self.collider_to_id.get(&handle)?,
            distance: hit.time_of_impact,
            point: sweep.contact(hit, dir),
            normal: hit.normal1,
        })
    }
}

/// The accepted colliders whose query-tree boxes the box `start` touches when
/// moved along the unit `dir` for up to `max_toi`: a ray from its center
/// against each tree node grown by its half-size, the way parry's own
/// composite shape cast prunes (#1881). Unlike the box around the whole
/// sweep, a long diagonal or unlimited cast only visits the nodes on its path.
pub(super) fn swept_candidates<'q>(
    queries: &'q QueryPipeline<'q>,
    start: Aabb,
    dir: Vec3,
    max_toi: f32,
) -> impl Iterator<Item = (ColliderHandle, &'q Collider)> + 'q {
    let half = start.half_extents();
    let ray = Ray::new(start.center(), dir);
    queries
        .bvh
        .leaves(move |node| {
            let node = node.aabb();
            let grown = Aabb::new(node.mins - half, node.maxs + half);
            grown.cast_local_ray(&ray, max_toi, true).is_some()
        })
        .filter_map(move |leaf| {
            let (collider, handle) = queries.colliders.get_unknown_gen(leaf)?;
            let accepted = queries.filter.test(queries.bodies, handle, collider);
            accepted.then_some((handle, collider))
        })
}
