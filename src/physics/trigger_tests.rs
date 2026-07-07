//! Integration tests for the trigger enter/stay/exit edges (#310): a kinematic
//! body walking into, resting in, and leaving a static trigger volume must
//! surface each edge exactly once from `PhysicsWorld::step`.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn box_collider(size: f32, is_trigger: bool) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::splat(size),
        },
        is_trigger,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

#[test]
fn step_surfaces_enter_stay_exit_edges() {
    let mut scene = Scene::new();

    let zone = scene.add_entity("Zone".to_string());
    scene.world.set_static(zone, true);
    scene.world.set_collider(zone, Some(box_collider(2.0, true));

    let walker = scene.add_entity("Walker".to_string());
    scene.world.transform_mut(walker).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    scene.world.set_collider(walker, Some(box_collider(1.0, false));
    e.rigidbody = Some(RigidBodyComponent {
        active: true,
        is_kinematic: true,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: crate::components::CollisionDetection::Discrete,
    });

    let pair = (zone.min(walker), zone.max(walker));
    let mut world = PhysicsWorld::from_scene(&scene);

    // Far apart: no trigger events at all.
    assert!(world.step(&mut scene, DT).is_empty());

    // Walk into the zone: the overlap's first tick is enter + stay.
    scene.world.transform_mut(walker).unwrap().position = Vec3::ZERO;
    let ev = world.step(&mut scene, DT);
    assert_eq!(ev.entered, vec![pair], "first overlapping tick enters");
    assert_eq!(ev.stayed, vec![pair], "stay covers the enter tick too");
    assert!(ev.exited.is_empty());

    // Stand still: stay only — enter must not repeat.
    let ev = world.step(&mut scene, DT);
    assert!(ev.entered.is_empty(), "enter fires once per overlap");
    assert_eq!(ev.stayed, vec![pair]);
    assert!(ev.exited.is_empty());

    // Walk out: the tick after the overlap ends is exit only.
    scene.world.transform_mut(walker).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    let ev = world.step(&mut scene, DT);
    assert!(ev.entered.is_empty());
    assert!(ev.stayed.is_empty(), "no stay once the overlap ended");
    assert_eq!(ev.exited, vec![pair], "leaving surfaces exit once");

    // Gone: quiet again.
    assert!(world.step(&mut scene, DT).is_empty());
}
