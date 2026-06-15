//! src/physics/mod.rs — physics front door.
//!
//! The body/collision simulation is rapier3d (see `world::PhysicsWorld`). Ray
//! queries — both the in-engine hitscan and the Lua `Physics.Raycast`/`Shoot`
//! bindings — now go through rapier/parry's query pipeline
//! (`PhysicsWorld::cast_ray` / `cast_ray_filtered`), so a script's cast and the
//! engine's cast agree for the same ray. The old AABB approximation
//! (`cast_ray_in_scene`) has been retired.

mod build;
mod character;
mod convert;
mod query;
mod world;

pub use world::PhysicsWorld;

use crate::core::scene::Scene;

/// The hitscan acceptance test shared by the engine hitscan and the Lua
/// `Physics.Raycast`/`Shoot` bindings: never hit a dead entity. This is the only
/// rule the engine imposes — it carries no notion of who the "shooter" is. A
/// script excludes its own entity (so a shot can't hit the shooter) by passing an
/// `ignore_id` to `Physics.Raycast`/`Shoot`; the engine never special-cases a
/// name. Routing both casts through this one predicate is what makes them return
/// identical hits for the same ray.
pub fn is_hittable(scene: &Scene, id: u32) -> bool {
    scene
        .get_entity(id)
        .is_some_and(|e| e.health.as_ref().is_none_or(|h| !h.is_dead))
}
