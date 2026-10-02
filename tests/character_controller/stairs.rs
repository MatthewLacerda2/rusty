//! Steps and slopes: what a walking move climbs, and what it is stopped or slid
//! back by.

use glam::{Quat, Vec3};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

use super::{add_box, add_character, add_floor, feet_of, pos_of, walk_for};

/// A floor with a block `height` tall spanning x in [2, 6], and a character at
/// the origin.
fn step_scene(height: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    let size = Vec3::new(4.0, height, 6.0);
    add_box(
        &mut scene,
        Vec3::new(4.0, height * 0.5, 0.0),
        size,
        Quat::IDENTITY,
    );
    let id = add_character(&mut scene, Vec3::ZERO);
    (scene, id)
}

/// A 0.2 m step is under the default 0.3 m step offset: the character walks up
/// onto it and keeps going.
#[test]
fn climbs_a_step_under_the_step_offset() {
    let (mut scene, id) = step_scene(0.2);
    let mut physics = PhysicsWorld::from_scene(&scene);
    let last = walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 75);
    let feet = feet_of(&scene, id);
    assert!(feet.x > 3.0, "walked onto the step, got x={}", feet.x);
    assert!(
        feet.y > 0.19,
        "standing on top of it, got feet y={}",
        feet.y
    );
    assert!(last.grounded, "grounded on the step");
}

/// A 0.5 m step is over the offset: it blocks like a wall.
#[test]
fn is_blocked_by_a_step_over_the_step_offset() {
    let (mut scene, id) = step_scene(0.5);
    let mut physics = PhysicsWorld::from_scene(&scene);
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 75);
    let p = pos_of(&scene, id);
    assert!(
        p.x < 1.5 + 1e-3,
        "stopped at the step's face, got x={}",
        p.x
    );
    assert!(feet_of(&scene, id).y < 0.2, "never climbed it");
}

/// A ramp rising along +x at `degrees`, its surface through the origin's
/// ground line at x = 1.
fn ramp_scene(degrees: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    let rot = Quat::from_rotation_z(degrees.to_radians());
    // A 20 m long, 1 m thick plank, its top face's lower end at (1, 0).
    let along = rot * Vec3::X;
    let normal = rot * Vec3::Y;
    let center = Vec3::new(1.0, 0.0, 0.0) + along * 10.0 - normal * 0.5;
    add_box(&mut scene, center, Vec3::new(20.0, 1.0, 6.0), rot);
    let id = add_character(&mut scene, Vec3::new(-1.0, 0.0, 0.0));
    (scene, id)
}

/// A 30° ramp is under the 45° slope limit: the character walks up it.
#[test]
fn walks_up_a_slope_under_the_limit() {
    let (mut scene, id) = ramp_scene(30.0);
    let mut physics = PhysicsWorld::from_scene(&scene);
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 120);
    let feet = feet_of(&scene, id);
    assert!(feet.y > 2.0, "climbed the ramp, got feet y={}", feet.y);
}

/// A 60° ramp is over the limit: walking into it never gets far up, and gravity
/// slides the character back down off it.
#[test]
fn slides_off_a_slope_over_the_limit() {
    let (mut scene, id) = ramp_scene(60.0);
    let mut physics = PhysicsWorld::from_scene(&scene);
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 120);
    let high = feet_of(&scene, id).y;
    assert!(
        high < 0.6,
        "could not climb the steep ramp, got feet y={high}"
    );
    walk_for(&mut physics, &mut scene, id, Vec3::ZERO, 120);
    let low = feet_of(&scene, id).y;
    assert!(low < 0.1, "slid back down to the floor, got feet y={low}");
}
