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
/// `Physics.Raycast`/`Shoot` bindings: never hit the Player (the shooter) or a
/// dead entity. Routing both casts through this one predicate is what makes them
/// return identical hits for the same ray.
pub fn is_hittable(scene: &Scene, id: u32) -> bool {
    scene
        .get_entity(id)
        .is_some_and(|e| e.name != "Player" && e.health.as_ref().is_none_or(|h| !h.is_dead))
}
