//! src/physics/query.rs — ray queries over the rapier world.
//!
//! Split out of `world` so the step/sync pipeline and the read-only ray casts stay
//! separately legible. Both the in-engine hitscan and the Lua
//! `Physics.Raycast` binding go through `cast_ray_filtered`, so a script's
//! cast and the engine's cast agree for the same ray.
//!
//! **Inactive entities are invisible to queries (#521).** rapier's query
//! pipeline ignores the enabled flag that deactivation sets, so every query in
//! the engine — these casts, the `spatial` family, the character sweep — filters
//! through [`is_live`]: one check, inside the physics world, that covers a
//! deactivated body *and* a deactivated compound part.

use glam::Vec3;
use rapier3d::prelude::*;

use super::convert::{from_na_vec, to_na_vec};
use super::world::PhysicsWorld;

/// Whether `collider` takes part in queries: it is enabled (a deactivated
/// compound part is not) and so is the body carrying it (a deactivated owner
/// disables its whole body). The flags are pushed each physics tick, the same
/// cadence at which the query pipeline sees poses.
pub(super) fn is_live(bodies: &RigidBodySet, collider: &Collider) -> bool {
    collider.is_enabled()
        && collider
            .parent()
            .and_then(|h| bodies.get(h))
            .is_none_or(RigidBody::is_enabled)
}

impl PhysicsWorld {
    /// Closest collider hit by `ray` within `max_toi`, as (entity id, toi).
    pub fn cast_ray(&self, origin: Vec3, dir: Vec3, max_toi: f32) -> Option<(u32, f32)> {
        self.cast_ray_filtered(origin, dir, max_toi, |_| true)
    }

    /// Like [`Self::cast_ray`], but skips every collider whose entity id is
    /// rejected by `accept` *during* parry's traversal — so the ray passes through
    /// excluded colliders and reports the nearest accepted hit instead of stopping
    /// (and missing) on an excluded one. This is the primitive the engine hitscan
    /// and the Lua bindings share via [`crate::physics::is_hittable`].
    pub fn cast_ray_filtered(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<(u32, f32)> {
        let ray = Ray::new(to_na_vec(origin).into(), to_na_vec(dir.normalize()));
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        self.query_pipeline
            .cast_ray(&self.bodies, &self.colliders, &ray, max_toi, true, filter)
            .and_then(|(handle, toi)| self.collider_to_id.get(&handle).map(|&id| (id, toi)))
    }

    /// Closest hit by `ray` within `max_toi`, returning the time-of-impact and the
    /// world-space surface normal at the hit point. Used by the particle system to
    /// reflect a bouncing particle off the surface it struck. `solid = true` so a
    /// ray starting inside a collider reports `toi = 0` rather than passing through.
    pub fn cast_ray_with_normal(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
    ) -> Option<(f32, Vec3)> {
        let ray = Ray::new(to_na_vec(origin).into(), to_na_vec(dir.normalize()));
        let predicate = |_: ColliderHandle, c: &Collider| is_live(&self.bodies, c);
        self.query_pipeline
            .cast_ray_and_get_normal(
                &self.bodies,
                &self.colliders,
                &ray,
                max_toi,
                true,
                QueryFilter::default().predicate(&predicate),
            )
            .map(|(_, intersection)| {
                (
                    intersection.time_of_impact,
                    from_na_vec(intersection.normal),
                )
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
