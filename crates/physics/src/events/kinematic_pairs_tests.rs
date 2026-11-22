use glam::Vec3;
use rapier3d::prelude::ActiveCollisionTypes;

use super::static_pairs_tests::{boxed, set_body, touching, wall_on_floor, DT};
use crate::assemble::ACTIVE_TYPES;
use crate::PhysicsWorld;
use rusty_scene::Scene;

#[test]
fn a_kinematic_wall_on_a_static_floor_keeps_no_contact_or_event() {
    let (mut scene, floor, wall) = wall_on_floor(false);
    // A kinematic neighbour overlapping the wall: kinematic↔kinematic.
    boxed(
        &mut scene,
        Vec3::new(0.5, 1.0, 0.0),
        Vec3::ONE,
        false,
        false,
    );
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        let ev = world.step(&mut scene, DT);
        assert!(ev.collisions.is_empty(), "{:?}", ev.collisions);
    }
    assert_eq!(
        touching(&world),
        0,
        "kinematic↔static and ↔kinematic: no pair"
    );
    // Turned dynamic in Play, the wall pairs with the floor (and its neighbour).
    set_body(&mut scene, wall, false);
    let key = (floor.min(wall), floor.max(wall));
    let entered = (0..30).any(|_| {
        let ev = world.step(&mut scene, DT).collisions;
        ev.entered.iter().any(|p| p.key() == key)
    });
    assert!(entered, "the wall made dynamic enters with the floor");
    assert!(touching(&world) >= 1);
}

/// A static floor, a dynamic unit box resting on it at the origin, and a
/// kinematic unit box `gap` along -x from it. Returns `(scene, floor, crate, mover)`.
fn mover_and_crate(gap: f32) -> (Scene, u32, u32, u32) {
    let mut scene = Scene::new();
    let floor = boxed(
        &mut scene,
        -Vec3::Y * 0.5,
        Vec3::new(20.0, 1.0, 20.0),
        true,
        false,
    );
    let crate_id = boxed(&mut scene, Vec3::Y * 0.5, Vec3::ONE, false, false);
    set_body(&mut scene, crate_id, false);
    let at = Vec3::new(-1.0 - gap, 0.55, 0.0);
    let mover = boxed(&mut scene, at, Vec3::ONE, false, false);
    set_body(&mut scene, mover, true);
    (scene, floor, crate_id, mover)
}

fn asleep(world: &PhysicsWorld, id: u32) -> bool {
    let h = world.id_to_body[&id];
    world.bodies[h].is_sleeping()
}

#[test]
fn a_kinematic_mover_wakes_and_pushes_a_sleeping_crate() {
    let (mut scene, _, crate_id, mover) = mover_and_crate(1.0);
    let mut world = PhysicsWorld::from_scene(&scene);
    let settled = (0..600).any(|_| {
        world.step(&mut scene, DT);
        asleep(&world, crate_id)
    });
    assert!(settled, "the crate falls asleep on the floor");
    let start = scene.world.transform(crate_id).unwrap().position;
    let key = (crate_id.min(mover), crate_id.max(mover));
    let mut entered = false;
    for _ in 0..60 {
        scene.world.transform_mut(mover).unwrap().position.x += 2.0 * DT;
        let ev = world.step(&mut scene, DT).collisions;
        entered |= ev.entered.iter().any(|p| p.key() == key);
    }
    let end = scene.world.transform(crate_id).unwrap().position;
    assert!(end.x > start.x + 0.5, "pushed: {start} -> {end}");
    assert!(entered, "the push raises OnCollisionEnter on the pair");
}

#[test]
fn a_resting_body_switched_kinematic_exits_on_the_next_step() {
    let (mut scene, floor, crate_id, _) = mover_and_crate(5.0);
    let key = (floor.min(crate_id), floor.max(crate_id));
    let mut world = PhysicsWorld::from_scene(&scene);
    let landed = (0..120).any(|_| {
        let ev = world.step(&mut scene, DT).collisions;
        ev.entered.iter().any(|p| p.key() == key)
    });
    assert!(landed, "the crate rests on the floor");
    world.step(&mut scene, DT);
    scene.world.rigidbody_mut(crate_id).unwrap().is_kinematic = true;
    let ev = world.step(&mut scene, DT).collisions;
    assert_eq!(ev.exited, vec![key], "neither side is dynamic any more");
    assert!(world.step(&mut scene, DT).collisions.is_empty());
}

#[test]
fn active_types_are_rapiers_default() {
    assert_eq!(ACTIVE_TYPES, ActiveCollisionTypes::default());
}
