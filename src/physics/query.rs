//! src/physics/query.rs — ray queries over the rapier world.
//!
//! Split out of `world` so the step/sync pipeline and the read-only ray casts stay
//! separately legible. Both the in-engine hitscan and the Lua
//! `Physics.Raycast`/`Shoot` bindings go through `cast_ray_filtered`, so a script's
//! cast and the engine's cast agree for the same ray.

use glam::Vec3;
use rapier3d::prelude::*;

use super::convert::to_na_vec;
use super::world::PhysicsWorld;

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
        let predicate = |handle: ColliderHandle, _: &Collider| {
            self.collider_to_id
                .get(&handle)
                .is_some_and(|&id| accept(id))
        };
        let filter = QueryFilter::default().predicate(&predicate);
        self.query_pipeline
            .cast_ray(&self.bodies, &self.colliders, &ray, max_toi, true, filter)
            .and_then(|(handle, toi)| self.collider_to_id.get(&handle).map(|&id| (id, toi)))
    }
}
