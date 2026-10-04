//! src/physics/ccd_tests.rs — the collision-detection-mode proving test (#321).
//!
//! One scene, one experiment: a small, fast dynamic sphere fired straight at a
//! thin wall it moves several times its own diameter past in a single fixed tick,
//! so no overlap ever exists at a tick boundary. Since rapier 0.36 every fast
//! dynamic body is swept against *static* geometry whatever its mode, so a static
//! wall stops it either way; what `Continuous` adds is the sweep against moving
//! (kinematic and dynamic) bodies — Unity's Continuous / ContinuousDynamic split.
//! Against a kinematic wall `Discrete` tunnels and `Continuous` stops: the flag
//! is live simulation behaviour, not stored data.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;
/// A thin wall centred at x = 4 (0.2 m thick, tall/wide enough not to miss).
const WALL_X: f32 = 4.0;

/// A thin wall at `WALL_X`: static, or a (stationary) kinematic body when
/// `moving`.
fn add_wall(scene: &mut Scene, moving: bool) {
    let wall = scene.add_entity("Wall".to_string());
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(WALL_X, 0.0, 0.0);
    if moving {
        scene.world.set_rigidbody(
            wall,
            Some(RigidBodyComponent {
                is_kinematic: true,
                use_gravity: false,
                ..crate::scene::authoring::defaults::default_rigidbody()
            }),
        );
    } else {
        scene.world.set_static(wall, true);
    }
    scene.world.set_collider(
        wall,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::new(0.2, 4.0, 4.0),
            },
            is_trigger: false,
            material: Default::default(),
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
}

/// Build a scene: a thin wall at `WALL_X` (see [`add_wall`]) and a fast dynamic
/// sphere at the origin flying toward it at 300 m/s (5 m per 60 Hz tick — far
/// more than the wall is thick), with the given collision-detection mode.
/// Returns `(scene, world, sphere_id)`.
fn wall_and_projectile(mode: CollisionDetection, moving: bool) -> (Scene, PhysicsWorld, u32) {
    let mut scene = Scene::new();
    add_wall(&mut scene, moving);

    let sphere = scene.add_entity("Bullet".to_string());
    scene.world.transform_mut(sphere).unwrap().position = Vec3::ZERO;
    scene.world.set_collider(
        sphere,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Sphere { radius: 0.25 },
            is_trigger: false,
            material: Default::default(),
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    scene.world.set_rigidbody(
        sphere,
        Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            // 300 m/s along +x ⇒ 5 m per tick; gravity off so motion stays on the
            // x axis and the test reads a single coordinate.
            velocity: Vec3::new(300.0, 0.0, 0.0),
            angular_velocity: Vec3::ZERO,
            use_gravity: false,
            collision_detection: mode,
        }),
    );

    let world = PhysicsWorld::from_scene(&scene);
    (scene, world, sphere)
}

/// Step the world a few ticks and read the sphere's final x.
fn final_x(mode: CollisionDetection, moving: bool) -> f32 {
    let (mut scene, mut world, sphere) = wall_and_projectile(mode, moving);
    for _ in 0..3 {
        world.step(&mut scene, DT);
    }
    scene.world.transform(sphere).map(|t| t.position.x).unwrap()
}

#[test]
fn a_static_wall_stops_the_sphere_in_either_mode() {
    // rapier sweeps fast bodies against static geometry by default, so even a
    // Discrete sphere is stopped at the wall's near face, never crossing it.
    for mode in [CollisionDetection::Discrete, CollisionDetection::Continuous] {
        let x = final_x(mode, false);
        assert!(
            x < WALL_X,
            "{mode:?} body should stop at the wall, ended at x={x}"
        );
    }
}

#[test]
fn discrete_tunnels_through_a_moving_wall() {
    // No tick boundary ever finds the sphere overlapping the kinematic wall and
    // nothing sweeps it there, so it sails straight through.
    let x = final_x(CollisionDetection::Discrete, true);
    assert!(
        x > WALL_X + 1.0,
        "Discrete body should tunnel past the wall, ended at x={x}"
    );
}

#[test]
fn continuous_stops_at_a_moving_wall() {
    // CCD sweeps the motion against moving bodies too and stops the sphere at the
    // wall's near face (~WALL_X - half_thickness - radius).
    let x = final_x(CollisionDetection::Continuous, true);
    assert!(
        x < WALL_X,
        "Continuous body should be stopped before the wall, ended at x={x}"
    );
}
