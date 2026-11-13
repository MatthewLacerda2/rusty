//! crates/physics/src/shape/decompose/refit_tests.rs — a body that changes type in Play
//! re-fits its concave mesh collider in place (#1886): pieces when it turns
//! dynamic, the exact trimesh when it turns kinematic or static again.

use glam::Vec3;

use crate::shape::fixture::{arch, floor, prop, steps};
use crate::PhysicsWorld;
use rusty_scene::Scene;

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
    let rb = rusty_scene::authoring::default_rigidbody();
    scene.world.set_rigidbody(id, Some(rb));
    steps(&mut scene, &mut world, 1);
    assert!(
        shape_of(&world, id).as_compound().is_some(),
        "dynamic: pieces"
    );
}

#[test]
fn a_refit_that_fails_warns_once_not_every_step() {
    let mut scene = Scene::new();
    floor(&mut scene);
    let id = prop(&mut scene, arch(), Vec3::new(0.0, 0.5, 0.0), true);
    let mut world = PhysicsWorld::from_scene(&scene);
    steps(&mut scene, &mut world, 1);
    // The mesh goes bad in Play; the body then turns dynamic and keeps moving.
    scene.world.mesh_mut(id).unwrap().vertices[3].position[1] = f32::NAN;
    rusty_test_support::logs::init();
    rusty_test_support::logs::take();
    let at = scene.world.transform(id).unwrap().position;
    scene.world.rigidbody_mut(id).unwrap().is_kinematic = false;
    steps(&mut scene, &mut world, 5);
    assert_ne!(at, scene.world.transform(id).unwrap().position, "it moves");
    let logged = rusty_test_support::logs::take();
    let named = (logged.iter())
        .filter(|m| m.contains(&format!("entity {id}:")))
        .count();
    // The console folds a repeat into "(×N)": one line, never folded.
    assert_eq!(named, 1, "{logged:?}");
    assert!(!logged.iter().any(|m| m.contains('×')), "{logged:?}");
}
