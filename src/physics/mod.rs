//! src/physics/mod.rs — physics front door.
//!
//! The body/collision simulation is rapier3d (see `world::PhysicsWorld`). Ray
//! queries — both the in-engine hitscan and the Lua `Physics.Raycast`/`Shoot`
//! bindings — now go through rapier/parry's query pipeline
//! (`PhysicsWorld::cast_ray` / `cast_ray_filtered`), so a script's cast and the
//! engine's cast agree for the same ray. The old AABB approximation
//! (`cast_ray_in_scene`) has been retired.

mod build;
#[cfg(test)]
mod build_tests;
mod character;
mod convert;
mod query;
mod world;

pub use world::PhysicsWorld;

use crate::scene::Scene;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::HealthComponent;

    #[test]
    fn dead_and_missing_entities_are_not_hittable() {
        let mut scene = Scene::new();
        let alive = scene.add_entity("Alive".to_string());
        let dead = scene.add_entity("Dead".to_string());
        scene.get_entity_mut(dead).unwrap().health = Some(HealthComponent {
            current_health: 0.0,
            max_health: 100.0,
            is_dead: true,
        });
        assert!(is_hittable(&scene, alive), "a live entity is hittable");
        assert!(!is_hittable(&scene, dead), "a dead entity is not hittable");
        assert!(
            !is_hittable(&scene, 9999),
            "a missing entity is not hittable"
        );
    }
}
