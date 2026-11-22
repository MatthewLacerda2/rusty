// Panic-free sim core: bare `.unwrap()` is denied here (use `?` or a documented `.expect(...)`).
#![deny(clippy::unwrap_used)]

mod assemble;
#[cfg(test)]
mod ccd_tests;
mod character;
mod collider_ids;
mod compound;
#[cfg(test)]
mod compound_tests;
mod contact;
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
mod wire;
mod world;
mod writes;

#[doc(hidden)]
pub use character::bench as character_bench;
pub use character::{can_stand, move_character, CharacterMove, ControllerHit};
pub use events::collision::{CollisionEvents, CollisionPair, Contact};
pub use events::trigger::TriggerEvents;
pub use follow::check_kept_bones;
pub use joints::JointBreak;
#[doc(hidden)]
pub use query::convex_fixture::turned_hull;
pub use query::sweep::Sweep;
pub use query::through::RayCrossing;
pub use query::RayHit;
pub use shape::triangles::{collider_world_triangles, ColliderTriangles};
pub use wire::{
    collider_wire, ColliderWire, ShapeWire, WireCache, MAX_SEGMENTS as MAX_WIRE_SEGMENTS,
};
pub use world::PhysicsWorld;

use rusty_scene::Scene;

/// Everything one physics tick surfaces to scripts; joint breaks ascend by joint entity.
#[derive(Debug, Default)]
pub struct PhysicsEvents {
    pub triggers: TriggerEvents,
    pub collisions: CollisionEvents,
    pub joint_breaks: Vec<JointBreak>,
    /// rapier panicked during this step and its world was rebuilt from the scene (release builds
    /// only): the step surfaced no other event.
    pub rebuilt: Option<Rebuilt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuilt {
    pub message: String,
    /// Steps in a row that panicked, this one included: a cause that stays in the
    /// scene panics every step.
    pub in_a_row: u32,
}

/// Shared by AI sight, audio occlusion, the script casts and damage cover, so they agree on a hit:
/// an entity is hittable iff it exists.
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
