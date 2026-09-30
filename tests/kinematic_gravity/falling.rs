//! Free fall, and gravity only for a rigidbody that opts in.

use super::{add_character, kinematic_body, pos_of, DT};
use glam::Vec3;
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

#[test]
fn kinematic_body_with_gravity_falls() {
    let mut scene = Scene::new();
    let id = add_character(
        &mut scene,
        Vec3::new(0.0, 10.0, 0.0),
        Some(kinematic_body(true)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    // Semi-implicit Euler over n ticks drops g·dt²·n(n+1)/2 ≈ 4.99 m after 1 s.
    // Pinning the magnitude (not just "went down") catches a broken integration.
    let drop = 10.0 - pos_of(&scene, id).y;
    assert!(
        (drop - 4.99).abs() < 0.1,
        "1 s free fall ≈ 4.99 m, got {drop}"
    );
}

#[test]
fn gravity_needs_a_rigidbody_opting_in() {
    let mut scene = Scene::new();
    // No rigidbody at all (collider-only scenery) and an explicit opt-out: neither falls.
    let bare = add_character(&mut scene, Vec3::new(0.0, 10.0, 0.0), None);
    let opted_out = add_character(
        &mut scene,
        Vec3::new(3.0, 10.0, 0.0),
        Some(kinematic_body(false)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    assert!(
        (pos_of(&scene, bare).y - 10.0).abs() < 1e-3,
        "no rigidbody: never falls"
    );
    assert!(
        (pos_of(&scene, opted_out).y - 10.0).abs() < 1e-3,
        "use_gravity=false: holds altitude"
    );
    // Flipping the authored flag at runtime takes effect next tick.
    scene.world.rigidbody_mut(opted_out).unwrap().use_gravity = true;
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    assert!(
        pos_of(&scene, opted_out).y < 9.0,
        "enabling use_gravity starts the fall"
    );
}
