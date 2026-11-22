use glam::{Quat, Vec3};
use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::query::{Ray, RayCast, ShapeCastHit, ShapeCastOptions};
use rapier3d::prelude::*;

use super::{sort_hits, RayHit};
use crate::convert::to_pose;
use crate::world::PhysicsWorld;

/// Sizes are trusted here: the Lua layer (`api::physics::cast::check`) refuses a negative
/// or non-finite one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sweep {
    /// A sphere.
    Sphere { center: Vec3, radius: f32 },
    /// A capsule between sphere centers `a` and `b`.
    Capsule { a: Vec3, b: Vec3, radius: f32 },
    Box {
        center: Vec3,
        half_extents: Vec3,
        rotation: Quat,
    },
}

impl Sweep {
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

    /// A sphere's contact is exact (one radius from its impact center, against the normal): parry's
    /// witness point carries GJK slop.
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

/// Prunes like parry's composite shape cast: a ray from the box center against each tree node grown
/// by the half-size.
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
