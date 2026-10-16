//! Queries: raycasts (plain and filtered) and trigger overlap pairs.

use super::{box_collider, dynamic_body};
use glam::Vec3;
use rusty::components::RigidBodyComponent;
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

#[test]
fn raycast_hits_nearest_collider() {
    let mut scene = Scene::new();
    let target = scene.add_entity("Target".to_string());
    scene.world.transform_mut(target).unwrap().position = Vec3::new(0.0, 0.0, 5.0);
    scene.world.set_static(target, true);
    scene
        .world
        .set_collider(target, Some(box_collider(Vec3::new(2.0, 2.0, 2.0), false)));
    let physics = PhysicsWorld::from_scene(&scene);
    let hit = physics.cast_ray(Vec3::ZERO, Vec3::Z, 100.0);
    assert_eq!(hit.map(|(id, _)| id), Some(target));
}

#[test]
fn cast_ray_filtered_skips_excluded_and_reports_next() {
    // Two boxes in a line along +Z. The filter rejecting the nearer one must make
    // the ray pass *through* it and report the farther — not miss. This is the
    // skip-and-continue behaviour the engine hitscan and Lua bindings share (#31).
    let mut scene = Scene::new();
    let near = scene.add_entity("Near".to_string());
    scene.world.transform_mut(near).unwrap().position = Vec3::new(0.0, 0.0, 3.0);
    scene.world.set_static(near, true);
    scene
        .world
        .set_collider(near, Some(box_collider(Vec3::new(2.0, 2.0, 2.0), false)));
    let far = scene.add_entity("Far".to_string());
    scene.world.transform_mut(far).unwrap().position = Vec3::new(0.0, 0.0, 8.0);
    scene.world.set_static(far, true);
    scene
        .world
        .set_collider(far, Some(box_collider(Vec3::new(2.0, 2.0, 2.0), false)));
    let physics = PhysicsWorld::from_scene(&scene);

    // Unfiltered: hits the nearer box.
    assert_eq!(
        physics
            .cast_ray(Vec3::ZERO, Vec3::Z, 100.0)
            .map(|(id, _)| id),
        Some(near),
    );
    // Reject the nearer box: the ray continues and reports the farther one.
    let hit = physics.cast_ray_filtered(Vec3::ZERO, Vec3::Z, 100.0, |id| id != near);
    assert_eq!(hit.map(|(id, _)| id), Some(far));
}

#[test]
fn trigger_overlap_reports_pair() {
    let mut scene = Scene::new();
    let a = scene.add_entity("Sensor".to_string());
    scene.world.set_static(a, true);
    scene
        .world
        .set_collider(a, Some(box_collider(Vec3::new(2.0, 2.0, 2.0), true)));
    let b = scene.add_entity("Walker".to_string());
    scene.world.transform_mut(b).unwrap().position = Vec3::new(0.0, 0.0, 0.0);
    scene
        .world
        .set_collider(b, Some(box_collider(Vec3::ONE, false)));
    scene.world.set_rigidbody(
        b,
        Some(RigidBodyComponent {
            is_kinematic: true,
            use_gravity: false,
            ..dynamic_body()
        }),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    let mut saw = false;
    for _ in 0..5 {
        let pairs = physics.step(&mut scene, 1.0 / 60.0);
        if pairs.triggers.stayed.contains(&(a, b)) {
            saw = true;
        }
    }
    assert!(saw, "overlapping sensor should report a trigger pair");
}
