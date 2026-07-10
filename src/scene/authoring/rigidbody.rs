//! src/scene/authoring/rigidbody.rs — Shared rigidbody-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `RigidBodyComponent` field by field: the `active` / `is_kinematic` / `use_gravity`
//! flags, the `mass` (clamped to a sane positive range), the linear/angular
//! `velocity`, and the `collision_detection` mode.
//!
//! BOTH the editor's RigidBody card and the Lua `Physics.*` field-setters
//! (`SetVelocity`, `SetAngularVelocity`, `SetKinematic`, `SetCollisionDetection`)
//! route through these, so the egui panel and the binding share one write (#287).
//! `Physics.AddForce` stays an integrator step in the API (it reads mass + the
//! kinematic gate to derive a velocity delta), not a field write, so it is not an
//! op here.
//!
//! Allowed deps: components (the `RigidBodyComponent` data). Pure.

use glam::Vec3;

use crate::components::{CollisionDetection, RigidBodyComponent};

/// Set the rigidbody's `active` flag.
pub fn set_active(rb: &mut RigidBodyComponent, active: bool) {
    rb.active = active;
}

/// Set the rigidbody's `is_kinematic` flag.
pub fn set_kinematic(rb: &mut RigidBodyComponent, is_kinematic: bool) {
    rb.is_kinematic = is_kinematic;
}

/// Set the rigidbody's `use_gravity` flag.
pub fn set_use_gravity(rb: &mut RigidBodyComponent, use_gravity: bool) {
    rb.use_gravity = use_gravity;
}

/// Set the rigidbody's mass, clamped to `[0.01, 1000.0]` (the editor card's range;
/// the clamp lives HERE, once, so a script set is bounded the same way).
pub fn set_mass(rb: &mut RigidBodyComponent, mass: f32) {
    rb.mass = mass.clamp(0.01, 1000.0);
}

/// Set the rigidbody's linear velocity.
pub fn set_velocity(rb: &mut RigidBodyComponent, velocity: Vec3) {
    rb.velocity = velocity;
}

/// Set the rigidbody's angular velocity (radians/sec per axis).
pub fn set_angular_velocity(rb: &mut RigidBodyComponent, angular_velocity: Vec3) {
    rb.angular_velocity = angular_velocity;
}

/// Set the rigidbody's collision-detection mode (Discrete / Continuous).
pub fn set_collision_detection(rb: &mut RigidBodyComponent, mode: CollisionDetection) {
    rb.collision_detection = mode;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::RigidBodyComponent;
    use crate::scene::Scene;

    fn scene_with_rb() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Body".to_string());
        let c = RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            use_gravity: true,
            collision_detection: CollisionDetection::Discrete,
        };
        scene.world.set_rigidbody(id, Some(c));
        (scene, id)
    }

    #[test]
    fn ops_write_through_with_mass_clamp() {
        let (mut scene, id) = scene_with_rb();
        let mut e = scene.world.rigidbody_mut(id).unwrap();
        set_active(&mut e, false);
        set_kinematic(&mut e, true);
        set_use_gravity(&mut e, false);
        set_mass(&mut e, 5000.0);
        set_velocity(&mut e, Vec3::new(1.0, 2.0, 3.0));
        set_angular_velocity(&mut e, Vec3::new(0.0, 4.0, 0.0));
        set_collision_detection(&mut e, CollisionDetection::Continuous);
        let rb = &*e;
        assert!(!rb.active);
        assert!(rb.is_kinematic);
        assert!(!rb.use_gravity);
        assert_eq!(rb.mass, 1000.0, "mass clamps to the upper bound");
        assert_eq!(rb.velocity, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(rb.angular_velocity, Vec3::new(0.0, 4.0, 0.0));
        assert_eq!(rb.collision_detection, CollisionDetection::Continuous);
    }
}
