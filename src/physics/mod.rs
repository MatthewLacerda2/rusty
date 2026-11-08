//! src/physics/mod.rs — physics front door.
//!
//! The body/collision simulation is rapier3d (see `world::PhysicsWorld`). Ray
//! queries — both the in-engine hitscan and the Lua `Physics.Raycast` binding —
//! go through rapier/parry's query pipeline (`PhysicsWorld::raycast_hit` /
//! `raycast_all`), so a script's cast and the engine's cast agree for the same
//! ray. The area-shaped queries (`spatial`: overlap, closest-point,
//! containment, #311) and the shape casts (`sweep`: sphere, capsule and box
//! casts, single or every hit, #1881) ride the same pipeline, so every spatial
//! answer comes from the one live world.

// Panic-free sim core (#195): bare `.unwrap()` is denied here (use `?` or a
// documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

mod assemble;
#[cfg(test)]
mod ccd_tests;
mod character;
mod compound;
#[cfg(test)]
mod compound_tests;
mod convert;
#[cfg(test)]
mod degenerate_parent_tests;
#[cfg(test)]
mod differs_tests;
mod events;
mod follow;
mod joints;
mod live;
#[cfg(test)]
mod live_tests;
mod material;
#[cfg(test)]
mod material_asset_tests;
#[cfg(test)]
mod material_tests;
mod query;
mod recover;
#[cfg(test)]
mod recover_joint_tests;
#[cfg(test)]
mod recover_tests;
mod shape;
mod sync;
mod watch;
mod world;
mod writes;

pub use character::{can_stand, move_character, CharacterMove};
pub use events::collision::{CollisionEvents, CollisionPair, Contact};
pub use events::trigger::TriggerEvents;
pub use joints::JointBreak;
#[cfg(test)]
pub(crate) use query::convex_fixture::turned_hull;
pub use query::sweep::Sweep;
pub use query::through::RayCrossing;
pub use query::RayHit;
pub use shape::triangles::{collider_world_triangles, ColliderTriangles};
pub use world::PhysicsWorld;

use crate::scene::Scene;

/// Everything one physics tick surfaces to scripts: the trigger overlaps
/// (`OnTrigger*`), the solid contacts (`OnCollision*`, #448) and the joints that
/// broke (`OnJointBreak`, #449), ascending by joint entity.
#[derive(Debug, Default)]
pub struct PhysicsEvents {
    pub triggers: TriggerEvents,
    pub collisions: CollisionEvents,
    pub joint_breaks: Vec<JointBreak>,
    /// rapier panicked during this step and its world was rebuilt from the scene
    /// (#1797, release builds only): the step surfaced no other event, and the
    /// caller logs it.
    pub rebuilt: Option<Rebuilt>,
}

/// A step rapier panicked in, its world rebuilt (#1797, see `recover`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuilt {
    /// The panic's message.
    pub message: String,
    /// Steps in a row that panicked, this one included: a cause that stays in the
    /// scene panics every step.
    pub in_a_row: u32,
}

/// The hitscan acceptance test shared by the engine hitscan and the Lua
/// `Physics.Raycast` binding: an entity is hittable iff it exists. The engine
/// imposes no other rule — it carries no notion of who the "shooter" is. A script
/// excludes its own entity (so a shot can't hit the shooter) by passing an
/// `ignore_id` to `Physics.Raycast`; the engine never special-cases a name.
/// Routing both casts through this one predicate is what makes them return
/// identical hits for the same ray.
pub fn is_hittable(scene: &Scene, id: u32) -> bool {
    scene.world.contains(id)
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
