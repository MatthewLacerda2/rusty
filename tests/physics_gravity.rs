//! Dynamic-body gravity opt-out (#209): `use_gravity = false` must exempt a body
//! from the world's −9.81, mirroring Unity's `Rigidbody.useGravity`. Pins the wiring
//! to rapier's `gravity_scale` at body build and on per-tick edit.

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

fn dynamic_body(use_gravity: bool) -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity,
        collision_detection: CollisionDetection::Discrete,
    }
}

fn falling_ball(scene: &mut Scene, use_gravity: bool) -> u32 {
    let id = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 10.0, 0.0);
    scene.world.set_collider(id, Some(box_collider(Vec3::ONE)));
    scene
        .world
        .set_rigidbody(id, Some(dynamic_body(use_gravity)));
    id
}

fn pos_y(scene: &Scene, id: u32) -> f32 {
    scene.world.transform(id).unwrap().position.y
}

#[test]
fn gravity_disabled_body_does_not_fall() {
    let mut scene = Scene::new();
    let id = falling_ball(&mut scene, false);
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..120 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    // With use_gravity = false the body holds its altitude (no integration of g).
    assert!(
        (pos_y(&scene, id) - 10.0).abs() < 1e-3,
        "use_gravity=false body should not fall, got y={}",
        pos_y(&scene, id),
    );
}

#[test]
fn gravity_enabled_body_falls() {
    // The complement of the above: the same body with use_gravity=true does fall,
    // so the test above is checking the flag, not an unconditional freeze.
    let mut scene = Scene::new();
    let id = falling_ball(&mut scene, true);
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..120 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    assert!(
        pos_y(&scene, id) < 9.0,
        "use_gravity=true body should fall, got y={}",
        pos_y(&scene, id),
    );
}

#[test]
fn toggling_use_gravity_at_runtime_takes_effect() {
    // gravity_scale is re-applied each tick, so flipping the flag mid-sim works.
    let mut scene = Scene::new();
    let id = falling_ball(&mut scene, false);
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    let held = pos_y(&scene, id);
    assert!((held - 10.0).abs() < 1e-3, "should still be held aloft");

    scene.world.rigidbody_mut(id).unwrap().use_gravity = true;
    for _ in 0..120 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    assert!(
        pos_y(&scene, id) < held - 0.5,
        "after enabling gravity the body should start falling, got y={}",
        pos_y(&scene, id),
    );
}
