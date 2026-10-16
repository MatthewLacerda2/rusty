//! Plain kinematic bodies are pure movers (#451, Unity's kinematic Rigidbody):
//! they go exactly where their Transform says — through walls, and never falling
//! — because collide-and-slide belongs to the `CharacterController` alone.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

fn box_collider(size: Vec3) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

fn kinematic_body(use_gravity: bool) -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: true,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity,
        collision_detection: CollisionDetection::Discrete,
    }
}

/// A static wall slab centred at x = 2 (spanning x in [1.5, 2.5]) and a unit-box
/// kinematic body at x = -1.
fn wall_and_body(scene: &mut Scene, use_gravity: bool) -> u32 {
    let wall = scene.add_entity("Wall".to_string());
    scene.world.set_static(wall, true);
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(2.0, 0.0, 0.0);
    let slab = box_collider(Vec3::new(1.0, 4.0, 8.0));
    scene.world.set_collider(wall, Some(slab));
    let body = scene.add_entity("Body".to_string());
    scene.world.transform_mut(body).unwrap().position = Vec3::new(-1.0, 0.0, 0.0);
    scene
        .world
        .set_collider(body, Some(box_collider(Vec3::ONE)));
    scene
        .world
        .set_rigidbody(body, Some(kinematic_body(use_gravity)));
    body
}

/// Driven straight at a wall, a kinematic body goes through it.
#[test]
fn kinematic_body_passes_through_a_static_wall() {
    let mut scene = Scene::new();
    let body = wall_and_body(&mut scene, false);
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        scene.world.transform_mut(body).unwrap().position.x += 0.2;
        physics.step(&mut scene, 1.0 / 60.0);
    }
    let x = scene.world.transform(body).unwrap().position.x;
    assert!(
        (x - 5.0).abs() < 1e-3,
        "went exactly where driven, got x={x}"
    );
}

/// `use_gravity` on a kinematic body does nothing: it stays where it is put.
#[test]
fn kinematic_body_ignores_use_gravity() {
    let mut scene = Scene::new();
    let body = wall_and_body(&mut scene, true);
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..60 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    let y = scene.world.transform(body).unwrap().position.y;
    assert_eq!(y, 0.0, "a kinematic body never falls");
}
