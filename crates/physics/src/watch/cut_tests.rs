//! A stated teleport (`World::cut_pose`) puts a dynamic body at its Transform, velocity kept
//! (`PhysicsWorld::warp_bodies`). Its cursor opens with the world, not with the lazy change
//! watch, so a teleport in a script's `Start` lands; a log it cannot read (an Undo step, a
//! clear) warps every dynamic body, since the Transform is then the authority.

use glam::Vec3;

use crate::PhysicsWorld;
use rusty_components::RigidBodyComponent;
use rusty_scene::create::{default_collider, default_rigidbody};
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// A weightless dynamic box at the origin moving at 10 m/s along +x, its physics just built.
fn drifting() -> (Scene, PhysicsWorld, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Box".to_string());
    scene.world.set_collider(id, Some(default_collider()));
    let rb = RigidBodyComponent {
        velocity: Vec3::new(10.0, 0.0, 0.0),
        use_gravity: false,
        drag: 0.0,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    let physics = PhysicsWorld::from_scene(&scene);
    (scene, physics, id)
}

fn moved(scene: &mut Scene, id: u32, to: Vec3) {
    scene.world.transform_mut(id).unwrap().position = to;
}

fn y(scene: &Scene, id: u32) -> f32 {
    scene.world.transform(id).unwrap().position.y
}

#[test]
fn a_teleport_before_the_first_step_lands_and_keeps_its_speed() {
    let (mut scene, mut physics, id) = drifting();
    moved(&mut scene, id, Vec3::new(0.0, 50.0, 0.0));
    scene.world.cut_pose(id);
    physics.step(&mut scene, DT);
    assert_eq!(y(&scene, id), 50.0, "warped, not returned to the origin");
    let x = scene.world.transform(id).unwrap().position.x;
    assert!((x - 10.0 * DT).abs() < 1e-3, "kept its 10 m/s: x {x}");
}

#[test]
fn a_teleport_after_many_steps_lands() {
    let (mut scene, mut physics, id) = drifting();
    for _ in 0..5 {
        physics.step(&mut scene, DT);
    }
    moved(&mut scene, id, Vec3::new(0.0, 50.0, 0.0));
    scene.world.cut_pose(id);
    physics.step(&mut scene, DT);
    assert_eq!(y(&scene, id), 50.0);
}

#[test]
fn a_plain_write_to_a_dynamic_body_is_not_a_teleport() {
    let (mut scene, mut physics, id) = drifting();
    physics.step(&mut scene, DT);
    moved(&mut scene, id, Vec3::new(0.0, 50.0, 0.0));
    physics.step(&mut scene, DT);
    assert_eq!(y(&scene, id), 0.0, "the solver owns a dynamic pose");
}

#[test]
fn a_log_nobody_can_read_puts_every_dynamic_body_where_its_transform_says() {
    let (mut scene, mut physics, id) = drifting();
    physics.step(&mut scene, DT);
    moved(&mut scene, id, Vec3::new(0.0, 50.0, 0.0));
    scene.world.cut_pose(id);
    scene.world.rewind_changes(); // an Undo step: every cursor reads Rebuild
    physics.step(&mut scene, DT);
    assert_eq!(y(&scene, id), 50.0, "the Transform is the authority");
}
