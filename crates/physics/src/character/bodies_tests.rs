use std::sync::Arc;

use rapier3d::prelude::*;

use crate::world::PhysicsWorld;
use rusty_scene::Scene;

/// The world's current controller copy.
fn copy(world: &PhysicsWorld) -> Arc<RigidBodySet> {
    world.controller_bodies.of(&world.bodies)
}

#[test]
fn shows_kinematics_fixed_and_keeps_dynamics() {
    let mut world = PhysicsWorld::from_scene(&Scene::new());
    let kin = world.insert_body(RigidBodyBuilder::kinematic_position_based().build());
    let dyn_ = world.insert_body(RigidBodyBuilder::dynamic().build());
    let view = copy(&world);
    assert!(view[kin].is_fixed());
    assert!(view[dyn_].is_dynamic());
    assert!(
        world.bodies[kin].is_kinematic(),
        "the world's own body is untouched"
    );
}

#[test]
fn reused_until_an_insert_remove_or_retype_remakes_it() {
    let mut world = PhysicsWorld::from_scene(&Scene::new());
    let body = world.insert_body(RigidBodyBuilder::dynamic().build());
    let first = copy(&world);
    assert!(Arc::ptr_eq(&first, &copy(&world)), "no change: reused");
    // A write that is no insert, remove or retype keeps the copy.
    world.bodies[body].set_linvel(Vector::new(1.0, 0.0, 0.0), true);
    assert!(
        Arc::ptr_eq(&first, &copy(&world)),
        "a velocity write: reused"
    );

    let other = world.insert_body(RigidBodyBuilder::fixed().build());
    let inserted = copy(&world);
    assert!(!Arc::ptr_eq(&first, &inserted), "an insert remakes it");
    assert!(inserted.contains(other));

    let class = RigidBodyType::KinematicPositionBased;
    world
        .controller_bodies
        .retype(&mut world.bodies[body], class);
    let retyped = copy(&world);
    assert!(!Arc::ptr_eq(&inserted, &retyped), "a retype remakes it");
    assert!(retyped[body].is_fixed());

    world.remove_body(other);
    let removed = copy(&world);
    assert!(!Arc::ptr_eq(&retyped, &removed), "a remove remakes it");
    assert!(!removed.contains(other));
}
