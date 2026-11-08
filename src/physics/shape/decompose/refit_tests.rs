//! src/physics/shape/decompose/refit_tests.rs — a body that changes type in Play
//! re-fits its concave mesh collider in place (#1886): pieces when it turns
//! dynamic, the exact trimesh when it turns kinematic or static again.

use glam::Vec3;

use crate::physics::shape::fixture::{arch, floor, prop, steps};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// The collider shape of `id`'s body, as rapier holds it.
fn shape_of(world: &PhysicsWorld, id: u32) -> rapier3d::prelude::SharedShape {
    let handle = world.id_to_collider[&id];
    world.colliders[handle].shared_shape().clone()
}

#[test]
fn a_kinematic_body_turned_dynamic_in_play_is_decomposed_and_back() {
    let mut scene = Scene::new();
    floor(&mut scene);
    let id = prop(&mut scene, arch(), Vec3::new(0.0, 0.5, 0.0), true);
    let mut world = PhysicsWorld::from_scene(&scene);
    steps(&mut scene, &mut world, 1);
    let handle = world.id_to_collider[&id];
    assert!(
        shape_of(&world, id).as_trimesh().is_some(),
        "kinematic: exact"
    );
    scene.world.rigidbody_mut(id).unwrap().is_kinematic = false;
    steps(&mut scene, &mut world, 1);
    assert_eq!(
        world.id_to_collider[&id], handle,
        "the same collider, re-fitted"
    );
    assert!(
        shape_of(&world, id).as_compound().is_some(),
        "dynamic: pieces"
    );
    let body = &world.bodies[world.id_to_body[&id]];
    assert!((body.mass() - 5.0).abs() < 1e-3, "mass {}", body.mass());
    scene.world.rigidbody_mut(id).unwrap().is_kinematic = true;
    steps(&mut scene, &mut world, 1);
    assert!(
        shape_of(&world, id).as_trimesh().is_some(),
        "kinematic again: exact"
    );
}

#[test]
fn a_static_mesh_given_a_rigidbody_in_play_is_decomposed() {
    let mut scene = Scene::new();
    floor(&mut scene);
    let id = prop(&mut scene, arch(), Vec3::new(0.0, 0.5, 0.0), false);
    scene.world.set_rigidbody(id, None);
    scene.world.set_static(id, true);
    let mut world = PhysicsWorld::from_scene(&scene);
    steps(&mut scene, &mut world, 1);
    assert!(shape_of(&world, id).as_trimesh().is_some(), "static: exact");
    scene.world.set_static(id, false);
    let rb = crate::scene::authoring::default_rigidbody();
    scene.world.set_rigidbody(id, Some(rb));
    steps(&mut scene, &mut world, 1);
    assert!(
        shape_of(&world, id).as_compound().is_some(),
        "dynamic: pieces"
    );
}
