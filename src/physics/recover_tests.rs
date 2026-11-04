//! src/physics/recover_tests.rs — the rapier world rebuilt from the scene (#1797,
//! `recover`): a panic forced through the test seam, under the release behaviour,
//! rebuilds the world deterministically; a debug build still panics; and a first
//! real step after a paused one (#1795) steps as a world that never paused.

use glam::Vec3;

use super::watch::fixture_tests::{digest, fixture, scene_digest};
use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// `steps` steps of the fixture, with a panic forced on step `panic_at`.
fn run(steps: usize, panic_at: usize) -> (Scene, PhysicsWorld, Vec<Option<String>>) {
    let mut scene = fixture();
    let mut world = PhysicsWorld::from_scene(&scene);
    world.recovery.catch = true;
    let mut rebuilt = Vec::new();
    for n in 0..steps {
        world.recovery.panic_next = n == panic_at;
        rebuilt.push(world.step(&mut scene, DT).rebuilt);
    }
    (scene, world, rebuilt)
}

#[test]
fn a_caught_panic_rebuilds_the_world_from_the_scene() {
    let (scene, world, rebuilt) = run(20, 19);
    assert_eq!(
        rebuilt[19].as_deref(),
        Some("a panic forced through the test seam")
    );
    assert!(
        rebuilt[..19].iter().all(Option::is_none),
        "only the panic reports"
    );
    let fresh = PhysicsWorld::from_scene(&scene);
    assert_eq!(digest(&world), digest(&fresh), "the world is the scene's");
}

#[test]
fn the_same_inputs_reach_the_same_panic_and_the_same_rebuild() {
    let (a_scene, a, a_rebuilt) = run(60, 20);
    let (b_scene, b, b_rebuilt) = run(60, 20);
    assert_eq!(a_rebuilt, b_rebuilt);
    assert_eq!(digest(&a), digest(&b));
    assert_eq!(scene_digest(&a_scene), scene_digest(&b_scene));
}

#[test]
#[should_panic(expected = "a panic forced through the test seam")]
fn a_debug_build_still_panics() {
    let mut scene = fixture();
    let mut world = PhysicsWorld::from_scene(&scene);
    world.recovery.panic_next = true;
    world.step(&mut scene, DT);
}

/// A static floor and a dynamic sphere already resting on it.
fn resting_sphere() -> Scene {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    scene.world.transform_mut(floor).unwrap().position = Vec3::new(0.0, -0.5, 0.0);
    scene.world.set_static(floor, true);
    let size = Vec3::new(20.0, 1.0, 20.0);
    let collider = |shape| ColliderComponent {
        shape,
        ..crate::scene::authoring::default_collider()
    };
    let box_shape = ColliderShape::Box { size };
    scene.world.set_collider(floor, Some(collider(box_shape)));
    let ball = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(ball).unwrap().position = Vec3::Y * 0.99;
    let sphere = ColliderShape::Sphere { radius: 1.0 };
    scene.world.set_collider(ball, Some(collider(sphere)));
    let body = RigidBodyComponent {
        is_kinematic: false,
        ..crate::scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(ball, Some(body));
    scene
}

/// How many contacts entered, and exited, over `dts`, with a panic forced on
/// step `panic_at`.
fn edges(dts: &[f32], panic_at: Option<usize>) -> (usize, usize) {
    let mut scene = resting_sphere();
    let mut world = PhysicsWorld::from_scene(&scene);
    world.recovery.catch = true;
    let (mut entered, mut exited) = (0, 0);
    for (n, &dt) in dts.iter().enumerate() {
        world.recovery.panic_next = panic_at == Some(n);
        let ev = world.step(&mut scene, dt).collisions;
        (entered, exited) = (entered + ev.entered.len(), exited + ev.exited.len());
    }
    (entered, exited)
}

#[test]
fn a_contact_enters_once_across_a_paused_start() {
    assert_eq!(edges(&[0.0, DT, DT], None), (1, 0));
    assert_eq!(edges(&[0.0, 0.0, DT, 0.0, DT], None), (1, 0));
}

/// Contact state starts over after a rebuild, as after a scene load: the
/// resting pair enters again and nothing exits.
#[test]
fn a_contact_enters_again_after_a_rebuild() {
    assert_eq!(edges(&[DT, DT, DT, DT], Some(2)), (2, 0));
}

/// The frame after unpausing steps as a world that never paused (#1795).
#[test]
fn a_paused_start_steps_as_one_that_never_paused() {
    let (mut paused, mut straight) = (fixture(), fixture());
    let mut p = PhysicsWorld::from_scene(&paused);
    let mut s = PhysicsWorld::from_scene(&straight);
    p.step(&mut paused, 0.0);
    p.step(&mut paused, 0.0);
    for _ in 0..30 {
        p.step(&mut paused, DT);
        s.step(&mut straight, DT);
        assert_eq!(digest(&p), digest(&s));
        assert_eq!(scene_digest(&paused), scene_digest(&straight));
    }
}
