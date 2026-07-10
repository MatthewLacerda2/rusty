//! rapier3d-backed physics: gravity, kinematic drive, static blocking, raycast.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

fn box_collider(size: Vec3, is_trigger: bool) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

fn dynamic_body() -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: true,
        collision_detection: CollisionDetection::Discrete,
    }
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

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
        if pairs.stayed.contains(&(a, b)) {
            saw = true;
        }
    }
    assert!(saw, "overlapping sensor should report a trigger pair");
}
