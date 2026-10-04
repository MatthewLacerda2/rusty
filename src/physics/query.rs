//! src/physics/query.rs — ray queries over the rapier world.
//!
//! Split out of `world` so the step/sync pipeline and the read-only ray casts stay
//! separately legible. The in-engine hitscan, the particles and the Lua
//! `Physics.Raycast` / `RaycastAll` bindings all go through `raycast_hit` /
//! `raycast_all`, so a script's cast and the engine's agree for the same ray.
//!
//! **Inactive entities are invisible to queries (#521).** rapier's query
//! pipeline ignores the enabled flag that deactivation sets, so every query in
//! the engine — these casts, the `spatial` family, the character sweep — filters
//! through [`is_live`]: one check, inside the physics world, that covers a
//! deactivated body *and* a deactivated compound part.

use glam::Vec3;
use rapier3d::prelude::*;

use super::world::PhysicsWorld;

/// Whether `collider` takes part in queries: it is enabled (a deactivated
/// compound part is not) and so is the body carrying it (a deactivated owner
/// disables its whole body). The flags are pushed each physics tick, the same
/// cadence at which the query tree sees poses.
pub(super) fn is_live(bodies: &RigidBodySet, collider: &Collider) -> bool {
    collider.is_enabled()
        && collider
            .parent()
            .and_then(|h| bodies.get(h))
            .is_none_or(RigidBody::is_enabled)
}

/// One ray hit: the entity struck, how far along the ray, the world-space point,
/// and the world-space outward surface normal there. Which bone and which
/// character a hit belongs to (#464) are scene facts, read off `id` by
/// `Scene::hit_bone` / `Scene::root_of`, so physics stays hierarchy-agnostic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    /// Entity id owning the collider hit — the compound *part*, not its body.
    pub id: u32,
    /// Distance from the origin along the normalized direction.
    pub distance: f32,
    /// World-space hit point.
    pub point: Vec3,
    /// World-space unit surface normal, facing out of the collider hit.
    pub normal: Vec3,
}

/// Order hits nearest-first, equal distances broken by entity id, so a
/// multi-hit answer never leaks parry's traversal order.
pub(super) fn sort_hits(hits: &mut [RayHit]) {
    hits.sort_by(|a, b| a.distance.total_cmp(&b.distance).then(a.id.cmp(&b.id)));
}

impl PhysicsWorld {
    /// A scene-query view of the live world under `filter`. rapier keeps the
    /// query tree inside the broad phase, refreshed at the end of every step;
    /// [`Self::refresh_query_tree`] covers moves made between steps.
    pub(super) fn queries<'a>(&'a self, filter: QueryFilter<'a>) -> QueryPipeline<'a> {
        self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            filter,
        )
    }

    /// Re-seat `handles` in the query tree at their current poses, so a query
    /// made before the next step sees them where they now are (scene load, a
    /// bone-carried collider). The next step folds the change in as usual.
    pub(super) fn refresh_query_tree(&mut self, handles: &[ColliderHandle]) {
        for &handle in handles {
            if let Some(collider) = self.colliders.get(handle) {
                let aabb =
                    collider.compute_broad_phase_aabb(&self.integration_parameters, &self.bodies);
                self.broad_phase
                    .set_aabb(&self.integration_parameters, handle, aabb);
            }
        }
    }

    /// Closest collider hit by `ray` within `max_toi`, as (entity id, toi).
    pub fn cast_ray(&self, origin: Vec3, dir: Vec3, max_toi: f32) -> Option<(u32, f32)> {
        self.cast_ray_filtered(origin, dir, max_toi, |_| true)
    }

    /// [`Self::raycast_hit`] reduced to (entity id, distance) — the shape the
    /// engine hitscan wants.
    pub fn cast_ray_filtered(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<(u32, f32)> {
        self.raycast_hit(origin, dir, max_toi, accept)
            .map(|hit| (hit.id, hit.distance))
    }

    /// Closest hit by `ray` within `max_toi`, returning the distance and the
    /// world-space surface normal. Used by the particle system to reflect a
    /// bouncing particle off the surface it struck. A ray starting inside a
    /// collider reports distance 0 rather than passing through.
    pub fn cast_ray_with_normal(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
    ) -> Option<(f32, Vec3)> {
        self.raycast_hit(origin, dir, max_toi, |_| true)
            .map(|hit| (hit.distance, hit.normal))
    }

    /// The nearest collider hit by the ray within `max_toi`, skipping every
    /// collider whose entity id `accept` rejects *during* parry's traversal — so
    /// the ray passes through excluded colliders and reports the nearest accepted
    /// hit instead of stopping (and missing) on an excluded one. The one ray
    /// primitive the engine hitscan, the particles and the Lua casts share.
    /// `solid`: a ray starting inside a collider hits it at distance 0.
    pub fn raycast_hit(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<RayHit> {
        let ray = Ray::new(origin, dir.normalize());
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        self.queries(filter)
            .cast_ray_and_get_normal(&ray, max_toi, true)
            .and_then(|(handle, hit)| self.ray_hit(&ray, handle, hit))
    }

    /// Every accepted collider the ray crosses within `max_toi`, nearest first
    /// (ties by entity id) — Unity's `Physics.RaycastAll`, the query wallbangs
    /// walk. Each collider reports its entry point (a ray starting inside one
    /// hits it at distance 0).
    pub fn raycast_all(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<RayHit> {
        let ray = Ray::new(origin, dir.normalize());
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let mut hits: Vec<RayHit> = self
            .queries(filter)
            .intersect_ray(ray, max_toi, true)
            .filter_map(|(handle, _, hit)| self.ray_hit(&ray, handle, hit))
            .collect();
        sort_hits(&mut hits);
        hits
    }

    /// The nearest solid surface *ahead* of the ray within `max_toi`: sensors
    /// (triggers) are passed through, and a collider the origin starts inside (the
    /// player's own capsule around the camera) is looked past rather than hit at 0.
    /// How the UI finds the wall in front of a world canvas (#429).
    pub fn first_surface_ahead(&self, origin: Vec3, dir: Vec3, max_toi: f32) -> Option<RayHit> {
        let ray = Ray::new(origin, dir.normalize());
        let accept = |_: u32| true;
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default()
            .exclude_sensors()
            .predicate(&predicate);
        let mut hits: Vec<RayHit> = self
            .queries(filter)
            .intersect_ray(ray, max_toi, true)
            .filter_map(|(handle, _, hit)| self.ray_hit(&ray, handle, hit))
            .collect();
        sort_hits(&mut hits);
        hits.into_iter().find(|h| h.distance > 1e-3)
    }

    /// Whether a solid collider lies on the segment `from → to` — the audio
    /// occlusion query (#467). Sensors (triggers) pass sound, and a collider the
    /// segment starts inside (the listener's own capsule) is looked past.
    pub fn segment_blocked(&self, from: Vec3, to: Vec3, accept: impl Fn(u32) -> bool) -> bool {
        let delta = to - from;
        let length = delta.length();
        if length <= 1e-3 {
            return false;
        }
        let ray = Ray::new(from, delta / length);
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default()
            .exclude_sensors()
            .predicate(&predicate);
        let blocked = self
            .queries(filter)
            .intersect_ray(ray, length, true)
            .any(|(_, _, hit)| hit.time_of_impact > 1e-3);
        blocked
    }

    /// Resolve a rapier ray intersection to a [`RayHit`]; `None` when the
    /// collider belongs to no entity.
    fn ray_hit(&self, ray: &Ray, handle: ColliderHandle, hit: RayIntersection) -> Option<RayHit> {
        let &id = self.collider_to_id.get(&handle)?;
        Some(RayHit {
            id,
            distance: hit.time_of_impact,
            point: ray.point_at(hit.time_of_impact),
            normal: hit.normal,
        })
    }

    /// Adapt an entity-id acceptance test to rapier's collider-handle predicate:
    /// a collider passes when it is [`is_live`] and its entity is accepted.
    pub(super) fn handle_accepts<'a>(
        &'a self,
        accept: &'a dyn Fn(u32) -> bool,
    ) -> impl Fn(ColliderHandle, &Collider) -> bool + 'a {
        move |handle, collider| {
            is_live(&self.bodies, collider)
                && self
                    .collider_to_id
                    .get(&handle)
                    .is_some_and(|&id| accept(id))
        }
    }
}
