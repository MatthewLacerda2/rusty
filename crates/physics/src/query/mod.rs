use glam::Vec3;
use rapier3d::prelude::*;

use crate::world::PhysicsWorld;

pub(super) mod convex_fixture;
#[cfg(test)]
mod convex_tests;
#[cfg(test)]
mod raycast_tests;
pub(super) mod spatial;
#[cfg(test)]
mod spatial_tests;
pub(super) mod sweep;
#[cfg(test)]
mod sweep_tests;
#[cfg(test)]
mod tests;
pub(super) mod through;
#[cfg(test)]
mod through_edge_tests;
#[cfg(test)]
mod through_tests;

/// rapier's query pipeline ignores the enabled flag: check the collider and its body (a deactivated
/// owner disables its whole body).
pub(super) fn is_live(bodies: &RigidBodySet, collider: &Collider) -> bool {
    collider.is_enabled()
        && collider
            .parent()
            .and_then(|h| bodies.get(h))
            .is_none_or(RigidBody::is_enabled)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    /// Entity id owning the collider hit — the compound *part*, not its body.
    pub id: u32,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

pub(super) fn sort_hits(hits: &mut [RayHit]) {
    hits.sort_by(|a, b| a.distance.total_cmp(&b.distance).then(a.id.cmp(&b.id)));
}

impl PhysicsWorld {
    pub(super) fn queries<'a>(&'a self, filter: QueryFilter<'a>) -> QueryPipeline<'a> {
        self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            filter,
        )
    }

    /// rapier refreshes the query tree only at step end; this covers moves between steps.
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

    pub fn cast_ray_with_normal(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
    ) -> Option<(f32, Vec3)> {
        self.raycast_hit(origin, dir, max_toi, |_| true)
            .map(|hit| (hit.distance, hit.normal))
    }

    /// `accept` filters during traversal, so rejected colliders are passed through; a ray starting
    /// inside a collider hits it at distance 0.
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

    /// Sensors are passed through; a collider the origin starts inside is looked past, not hit
    /// at 0.
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

    /// Sensors pass sound; a collider the segment starts inside is looked past.
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

    fn ray_hit(&self, ray: &Ray, handle: ColliderHandle, hit: RayIntersection) -> Option<RayHit> {
        let &id = self.collider_to_id.get(&handle)?;
        Some(RayHit {
            id,
            distance: hit.time_of_impact,
            point: ray.point_at(hit.time_of_impact),
            normal: hit.normal,
        })
    }

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
