//! Continuous collision detection (CCD, #321): a small, fast dynamic body
//! tunnels clean through a thin static wall when left `Discrete`, but is
//! stopped at the wall once switched to `Continuous` — the proof that the
//! `collision_detection` flag actually drives rapier's dormant `CCDSolver`
//! rather than just sitting on the component.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

fn box_collider(size: Vec3) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

fn sphere_collider(radius: f32) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Sphere { radius },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

/// A small, very fast bullet body (200 m/s along +Z) — its per-tick step at a
/// fixed 60Hz dt (~3.3 units) is over an order of magnitude past the wall's
/// 0.05-unit thickness, so a discrete final-pose-only test never sees an
/// overlap.
fn fast_bullet(mode: CollisionDetection) -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::new(0.0, 0.0, 200.0),
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: mode,
    }
}

/// A thin static wall at z=5, and a fast dynamic sphere fired at it from
/// z=0 with the given collision-detection mode. Steps 10 fixed 1/60s ticks
/// (matching the deterministic FixedUpdate harness) and returns the sphere's
/// final z.
fn fire_at_wall(mode: CollisionDetection) -> f32 {
    let mut scene = Scene::new();
    let wall = scene.add_entity("Wall".to_string());
    {
        let mut e = scene.get_entity_mut(wall).unwrap();
        e.transform.position = Vec3::new(0.0, 0.0, 5.0);
        e.is_static = true;
        e.collider = Some(box_collider(Vec3::new(3.0, 3.0, 0.05)));
    }
    let bullet = scene.add_entity("Bullet".to_string());
    {
        let mut e = scene.get_entity_mut(bullet).unwrap();
        e.transform.position = Vec3::new(0.0, 0.0, 0.0);
        e.collider = Some(sphere_collider(0.1));
        e.rigidbody = Some(fast_bullet(mode));
    }
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..10 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    z_of(&scene, bullet)
}

fn z_of(scene: &Scene, id: u32) -> f32 {
    scene.get_entity(id).unwrap().transform.position.z
}

#[test]
fn discrete_fast_body_tunnels_through_thin_wall() {
    let z = fire_at_wall(CollisionDetection::Discrete);
    assert!(
        z > 10.0,
        "expected the bullet to tunnel past the wall, got z={z}"
    );
}

#[test]
fn continuous_fast_body_is_stopped_by_thin_wall() {
    let z = fire_at_wall(CollisionDetection::Continuous);
    assert!(
        z < 4.9,
        "expected CCD to stop the bullet at the wall, got z={z}"
    );
}
