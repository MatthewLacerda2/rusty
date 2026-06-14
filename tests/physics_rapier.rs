//! rapier3d-backed physics: gravity, kinematic drive, static blocking, raycast.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty::core::scene::Scene;
use rusty::physics::PhysicsWorld;

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
        use_gravity: true,
    }
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.get_entity(id).unwrap().transform.position
}

#[test]
fn dynamic_body_falls_under_gravity() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Ball".to_string());
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        e.transform.position = Vec3::new(0.0, 10.0, 0.0);
        e.collider = Some(box_collider(Vec3::ONE, false));
        e.rigidbody = Some(dynamic_body());
    }
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
    {
        let mut e = scene.get_entity_mut(floor).unwrap();
        e.is_static = true;
        e.collider = Some(box_collider(Vec3::new(20.0, 0.5, 20.0), false));
    }
    let ball = scene.add_entity("Ball".to_string());
    {
        let mut e = scene.get_entity_mut(ball).unwrap();
        e.transform.position = Vec3::new(0.0, 3.0, 0.0);
        e.collider = Some(box_collider(Vec3::ONE, false));
        e.rigidbody = Some(dynamic_body());
    }
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
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        e.transform.position = Vec3::new(0.0, 1.0, 0.0);
        e.collider = Some(box_collider(Vec3::ONE, false));
        e.rigidbody = Some(RigidBodyComponent {
            is_kinematic: true,
            use_gravity: false,
            ..dynamic_body()
        });
    }
    let mut physics = PhysicsWorld::from_scene(&scene);
    // Externally drive the kinematic body (as input/scripts do) each tick.
    for _ in 0..10 {
        {
            let mut e = scene.get_entity_mut(id).unwrap();
            e.transform.position.x += 0.1;
        }
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
    {
        let mut e = scene.get_entity_mut(target).unwrap();
        e.transform.position = Vec3::new(0.0, 0.0, 5.0);
        e.is_static = true;
        e.collider = Some(box_collider(Vec3::new(2.0, 2.0, 2.0), false));
    }
    let physics = PhysicsWorld::from_scene(&scene);
    let hit = physics.cast_ray(Vec3::ZERO, Vec3::Z, 100.0);
    assert_eq!(hit.map(|(id, _)| id), Some(target));
}

#[test]
fn trigger_overlap_reports_pair() {
    let mut scene = Scene::new();
    let a = scene.add_entity("Sensor".to_string());
    {
        let mut e = scene.get_entity_mut(a).unwrap();
        e.is_static = true;
        e.collider = Some(box_collider(Vec3::new(2.0, 2.0, 2.0), true));
    }
    let b = scene.add_entity("Walker".to_string());
    {
        let mut e = scene.get_entity_mut(b).unwrap();
        e.transform.position = Vec3::new(0.0, 0.0, 0.0);
        e.collider = Some(box_collider(Vec3::ONE, false));
        e.rigidbody = Some(RigidBodyComponent {
            is_kinematic: true,
            use_gravity: false,
            ..dynamic_body()
        });
    }
    let mut physics = PhysicsWorld::from_scene(&scene);
    let mut saw = false;
    for _ in 0..5 {
        let pairs = physics.step(&mut scene, 1.0 / 60.0);
        if pairs.contains(&(a, b)) {
            saw = true;
        }
    }
    assert!(saw, "overlapping sensor should report a trigger pair");
}
