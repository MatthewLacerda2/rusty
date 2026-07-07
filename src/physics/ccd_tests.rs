//! src/physics/ccd_tests.rs — the collision-detection-mode proving test (#321).
//!
//! One scene, one experiment: a small, fast dynamic sphere fired straight at a
//! thin static wall it moves several times its own diameter past in a single
//! fixed tick. Under `Discrete` no overlap ever exists at a tick boundary, so the
//! sphere tunnels through; under `Continuous` rapier's CCD sweep stops it at the
//! wall. That difference is the whole feature — the flag is live simulation
//! behaviour, not stored data.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;
/// A thin static wall centred at x = 4 (0.2 m thick, tall/wide enough not to miss).
const WALL_X: f32 = 4.0;

/// Build a scene: a thin static wall at `WALL_X` and a fast dynamic sphere at the
/// origin flying toward it at 300 m/s (5 m per 60 Hz tick — far more than the wall
/// is thick), with the given collision-detection mode. Returns `(scene, world,
/// sphere_id)`.
fn wall_and_projectile(mode: CollisionDetection) -> (Scene, PhysicsWorld, u32) {
    let mut scene = Scene::new();

    let wall = scene.add_entity("Wall".to_string());
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(WALL_X, 0.0, 0.0);
    scene.world.set_static(wall, true);
    scene.world.set_collider(wall, Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::new(0.2, 4.0, 4.0),
        },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    });

    let sphere = scene.add_entity("Bullet".to_string());
    scene.world.transform_mut(sphere).unwrap().position = Vec3::ZERO;
    scene.world.set_collider(sphere, Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Sphere { radius: 0.25 },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    });
    e.rigidbody = Some(RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        // 300 m/s along +x ⇒ 5 m per tick; gravity off so motion stays on the
        // x axis and the test reads a single coordinate.
        velocity: Vec3::new(300.0, 0.0, 0.0),
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: mode,
    });

    let world = PhysicsWorld::from_scene(&scene);
    (scene, world, sphere)
}

/// Step the world a few ticks and read the sphere's final x.
fn final_x(mode: CollisionDetection) -> f32 {
    let (mut scene, mut world, sphere) = wall_and_projectile(mode);
    for _ in 0..3 {
        world.step(&mut scene, DT);
    }
    scene.world.transform(sphere).map(|t| t.position.x).unwrap()
}

#[test]
fn discrete_tunnels_through_the_thin_wall() {
    // No tick boundary ever finds the sphere overlapping the wall, so it sails
    // straight through and ends up well past it.
    let x = final_x(CollisionDetection::Discrete);
    assert!(
        x > WALL_X + 1.0,
        "Discrete body should tunnel past the wall, ended at x={x}"
    );
}

#[test]
fn continuous_stops_at_the_thin_wall() {
    // CCD sweeps the motion and stops the sphere at the wall's near face
    // (~WALL_X - half_thickness - radius), never crossing it.
    let x = final_x(CollisionDetection::Continuous);
    assert!(
        x < WALL_X,
        "Continuous body should be stopped before the wall, ended at x={x}"
    );
}
