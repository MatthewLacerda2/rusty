//! src/physics/mod.rs — physics front door.
//!
//! The body/collision simulation is rapier3d (see `world::PhysicsWorld`). Ray
//! queries — both the in-engine hitscan and the Lua `Physics.Raycast` binding —
//! now go through rapier/parry's query pipeline
//! (`PhysicsWorld::cast_ray` / `cast_ray_filtered`), so a script's cast and the
//! engine's cast agree for the same ray. The old AABB approximation
//! (`cast_ray_in_scene`) has been retired. The area-shaped queries (`spatial`:
//! overlap, sphere-cast, closest-point, containment, #311) ride the same
//! pipeline, so every spatial answer comes from the one live world.

// Panic-free sim core (#195): bare `.unwrap()` is denied here (use `?` or a
// documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

mod build;
#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod ccd_tests;
mod character;
mod convert;
mod query;
mod spatial;
#[cfg(test)]
mod spatial_tests;
mod trigger_events;
#[cfg(test)]
mod trigger_tests;
mod world;

pub use trigger_events::TriggerEvents;
pub use world::PhysicsWorld;

use crate::scene::Scene;

/// The hitscan acceptance test shared by the engine hitscan and the Lua
/// `Physics.Raycast` binding: an entity is hittable iff it exists. The engine
/// imposes no other rule — it carries no notion of who the "shooter" is. A script
/// excludes its own entity (so a shot can't hit the shooter) by passing an
/// `ignore_id` to `Physics.Raycast`; the engine never special-cases a name.
/// Routing both casts through this one predicate is what makes them return
/// identical hits for the same ray.
pub fn is_hittable(scene: &Scene, id: u32) -> bool {
    scene.get_entity(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_entities_are_not_hittable() {
        let mut scene = Scene::new();
        let alive = scene.add_entity("Alive".to_string());
        assert!(is_hittable(&scene, alive), "an existing entity is hittable");
        assert!(
            !is_hittable(&scene, 9999),
            "a missing entity is not hittable"
        );
    }
}
