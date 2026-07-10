//! Kinematic character controller: bodies driven into static walls are blocked.

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

fn kinematic_body() -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: true,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: CollisionDetection::Discrete,
    }
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

/// A kinematic body driven straight into a static wall ends up *outside* the wall
/// (collide-and-slide blocks it) rather than tunneling through.
#[test]
fn kinematic_body_blocked_by_static_wall() {
    let mut scene = Scene::new();
    // Wall: thin slab centered at x = 2, spanning x in [1.5, 2.5].
    let wall = scene.add_entity("Wall".to_string());
    scene.world.set_static(wall, true);
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(2.0, 0.0, 0.0);
    scene
        .world
        .set_collider(wall, Some(box_collider(Vec3::new(1.0, 4.0, 8.0), false)));
    // Player: unit box starting well to the wall's left.
    let player = scene.add_entity("Player".to_string());
    scene.world.transform_mut(player).unwrap().position = Vec3::new(-1.0, 0.0, 0.0);
    scene
        .world
        .set_collider(player, Some(box_collider(Vec3::ONE, false)));
    scene.world.set_rigidbody(player, Some(kinematic_body()));
    let mut physics = PhysicsWorld::from_scene(&scene);
    // Drive the player hard into the wall each tick (as input/scripts do).
    for _ in 0..120 {
        scene.world.transform_mut(player).unwrap().position.x += 0.2;
        physics.step(&mut scene, 1.0 / 60.0);
    }
    // Player half-extent 0.5 + wall left face at x = 1.5 => center must stay below 1.0.
    let x = pos_of(&scene, player).x;
    assert!(
        x < 1.0,
        "kinematic body should be blocked outside the wall, got x={x}"
    );
}

/// Sliding: a body driven diagonally into a wall keeps moving along the wall's
/// free axis instead of stopping dead.
#[test]
fn kinematic_body_slides_along_wall() {
    let mut scene = Scene::new();
    let wall = scene.add_entity("Wall".to_string());
    scene.world.set_static(wall, true);
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(2.0, 0.0, 0.0);
    scene
        .world
        .set_collider(wall, Some(box_collider(Vec3::new(1.0, 4.0, 8.0), false)));
    let player = scene.add_entity("Player".to_string());
    scene.world.transform_mut(player).unwrap().position = Vec3::new(-1.0, 0.0, 0.0);
    scene
        .world
        .set_collider(player, Some(box_collider(Vec3::ONE, false)));
    scene.world.set_rigidbody(player, Some(kinematic_body()));
    let mut physics = PhysicsWorld::from_scene(&scene);
    // Few enough ticks that the body stays alongside the wall (half-z = 4) the
    // whole time rather than sliding off its end.
    for _ in 0..30 {
        // Push into the wall (+x) and along it (+z).
        scene.world.transform_mut(player).unwrap().position.x += 0.2;
        scene.world.transform_mut(player).unwrap().position.z += 0.1;
        physics.step(&mut scene, 1.0 / 60.0);
    }
    let p = pos_of(&scene, player);
    assert!(p.x < 1.0, "blocked on x by the wall, got x={}", p.x);
    assert!(p.z > 2.0, "should have slid along z, got z={}", p.z);
}
