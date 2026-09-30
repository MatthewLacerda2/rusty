//! Landing on ground, resting still, and a fresh fall off a ledge.

use super::{add_character, add_floor, kinematic_body, pos_of, DT};
use glam::Vec3;
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

#[test]
fn kinematic_body_lands_and_rests_on_ground() {
    let mut scene = Scene::new();
    add_floor(&mut scene, Vec3::new(20.0, 1.0, 20.0));
    let id = add_character(
        &mut scene,
        Vec3::new(0.0, 5.0, 0.0),
        Some(kinematic_body(true)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..300 {
        physics.step(&mut scene, DT);
    }
    // Landed: floor top 0.5 + character half-height 0.5 (+ controller offset).
    let rested = pos_of(&scene, id).y;
    assert!(
        (0.99..=1.10).contains(&rested),
        "should rest on the floor, got y={rested}"
    );
    // Grounded: it stays put — no sinking, no downward-speed jitter.
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    let still = pos_of(&scene, id).y;
    assert!(
        (still - rested).abs() < 1e-3,
        "resting body must not jitter/sink, got y={still}"
    );
}

#[test]
fn grounded_body_starts_a_fresh_fall_off_a_ledge() {
    let mut scene = Scene::new();
    // Narrow platform: x spans [-2, 2].
    add_floor(&mut scene, Vec3::new(4.0, 1.0, 4.0));
    let id = add_character(
        &mut scene,
        Vec3::new(0.0, 1.2, 0.0),
        Some(kinematic_body(true)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    // Rest on the platform long enough that un-zeroed gravity would have built
    // a large fall speed (~20 m/s after 2 s).
    for _ in 0..120 {
        physics.step(&mut scene, DT);
    }
    let rested = pos_of(&scene, id).y;
    // Step off the ledge (a script teleport) and tick once: the first tick of a
    // *fresh* fall drops ~g·dt² ≈ 0.003 m; a carried speed would plummet ~0.33 m.
    scene.world.transform_mut(id).unwrap().position.x = 5.0;
    physics.step(&mut scene, DT);
    let after = pos_of(&scene, id).y;
    assert!(
        rested - after < 0.05,
        "grounded ticks must not bank fall speed, dropped {}",
        rested - after
    );
    // And off the ledge it does keep falling.
    for _ in 0..120 {
        physics.step(&mut scene, DT);
    }
    assert!(
        pos_of(&scene, id).y < -5.0,
        "off the ledge the body falls freely"
    );
}
