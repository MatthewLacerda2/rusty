//! Autostep ignores dynamic bodies (rapier `include_dynamic_bodies: false`).

use glam::{Quat, Vec3};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

use super::{add_box, add_character, add_floor, feet_of, pos_of, walk_for};

fn make_dynamic(scene: &mut Scene, id: u32) {
    scene.world.set_static(id, false);
    let body = rusty::components::RigidBodyComponent {
        mass: 1000.0,
        ..rusty::edit::authoring::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(body));
}

fn add_step(scene: &mut Scene, edge: f32) {
    let size = Vec3::new(4.0, 0.2, edge + 3.0);
    let center = Vec3::new(4.0, 0.1, (edge - 3.0) * 0.5);
    add_box(scene, center, size, Quat::IDENTITY);
}

#[test]
fn never_climbs_a_dynamic_block() {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    let size = Vec3::new(4.0, 0.2, 6.0);
    let block = add_box(&mut scene, Vec3::new(4.0, 0.1, 0.0), size, Quat::IDENTITY);
    make_dynamic(&mut scene, block);
    let id = add_character(&mut scene, Vec3::ZERO);
    let mut physics = PhysicsWorld::from_scene(&scene);
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 75);
    let feet = feet_of(&scene, id);
    assert!(feet.y < 0.1, "climbed it: {feet}");
    let face = pos_of(&scene, block).x - 2.0;
    let x = pos_of(&scene, id).x;
    assert!(x < face - 0.3, "on top of it, face at {face}: x={x}");
}

/// A static step climbed while sliding along a dynamic crate's side is climbed
/// as without the crate: the crate is beside the walk, not what it stepped onto.
#[test]
fn climbs_a_static_step_beside_a_dynamic_crate() {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    add_step(&mut scene, 0.5);
    let size = Vec3::new(4.0, 1.0, 1.0);
    let crate_ = add_box(&mut scene, Vec3::new(2.0, 0.5, 1.05), size, Quat::IDENTITY);
    make_dynamic(&mut scene, crate_);
    let id = add_character(&mut scene, Vec3::new(0.0, 0.0, -0.05));
    let mut physics = PhysicsWorld::from_scene(&scene);
    let last = walk_for(&mut physics, &mut scene, id, Vec3::new(4.0, 0.0, 0.5), 75);
    let feet = feet_of(&scene, id);
    assert!(feet.x > 3.0, "walked onto the step: {feet}");
    assert!(feet.y > 0.19, "standing on top of it: {feet}");
    assert!(last.grounded, "grounded on the step");
}

/// A can on the tread, just past the step's edge, is in the way of autostep's
/// room check but is a dynamic body, which that check ignores: the walk still
/// steps up, and the can then stops it on the tread.
#[test]
fn a_can_on_the_tread_does_not_refuse_the_step() {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    add_step(&mut scene, 3.0);
    let size = Vec3::new(0.3, 0.6, 0.3);
    let can = add_box(&mut scene, Vec3::new(2.6, 0.5, 0.0), size, Quat::IDENTITY);
    make_dynamic(&mut scene, can);
    let id = add_character(&mut scene, Vec3::ZERO);
    let mut physics = PhysicsWorld::from_scene(&scene);
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 75);
    let feet = feet_of(&scene, id);
    assert!(feet.y > 0.19, "stepped up onto the tread: {feet}");
}
