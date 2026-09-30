//! Body simulation: gravity, static blocking, kinematic drive.

use super::{box_collider, dynamic_body, pos_of};
use glam::Vec3;
use rusty::components::RigidBodyComponent;
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

#[test]
fn dynamic_body_falls_under_gravity() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 10.0, 0.0);
    scene
        .world
        .set_collider(id, Some(box_collider(Vec3::ONE, false)));
    scene.world.set_rigidbody(id, Some(dynamic_body()));
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    assert!(pos_of(&scene, id).y < 9.9, "dynamic body should fall");
}

#[test]
fn static_floor_blocks_falling_body() {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    scene.world.set_static(floor, true);
    scene
        .world
        .set_collider(floor, Some(box_collider(Vec3::new(20.0, 0.5, 20.0), false)));
    let ball = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(ball).unwrap().position = Vec3::new(0.0, 3.0, 0.0);
    scene
        .world
        .set_collider(ball, Some(box_collider(Vec3::ONE, false)));
    scene.world.set_rigidbody(ball, Some(dynamic_body()));
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..240 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    // Rests on the floor (half-box 0.5 + floor top 0.25), never tunnels through.
    let y = pos_of(&scene, ball).y;
    assert!(y > 0.0, "body should rest above the floor, got y={y}");
}

#[test]
fn kinematic_body_follows_its_transform() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Player".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 1.0, 0.0);
    scene
        .world
        .set_collider(id, Some(box_collider(Vec3::ONE, false)));
    scene.world.set_rigidbody(
        id,
        Some(RigidBodyComponent {
            is_kinematic: true,
            use_gravity: false,
            ..dynamic_body()
        }),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    // Externally drive the kinematic body (as input/scripts do) each tick.
    for _ in 0..10 {
        scene.world.transform_mut(id).unwrap().position.x += 0.1;
        physics.step(&mut scene, 1.0 / 60.0);
    }
    let p = pos_of(&scene, id);
    assert!((p.x - 1.0).abs() < 1e-3, "kinematic x should track input");
    assert!((p.y - 1.0).abs() < 1e-3, "kinematic body ignores gravity");
}
