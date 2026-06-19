//! Ray queries: hit entity + distance, surface normal, and miss semantics.
//!
//! These assert the *values* the casts return (which entity, how far, the facing
//! normal) and that a clear miss is `None`, so a cast collapsed to a constant hit is
//! caught.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

/// A static box of `size` centred at `pos`; returns its entity id.
fn static_box(scene: &mut Scene, pos: Vec3, size: Vec3) -> u32 {
    let id = scene.add_entity("Box".to_string());
    let mut e = scene.get_entity_mut(id).unwrap();
    e.is_static = true;
    e.transform.position = pos;
    e.collider = Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    });
    id
}

#[test]
fn cast_ray_reports_entity_and_distance() {
    let mut scene = Scene::new();
    // Box centre z=5, size 2 -> half-extent 1 -> front face at z=4.
    let target = static_box(&mut scene, Vec3::new(0.0, 0.0, 5.0), Vec3::splat(2.0));
    let physics = PhysicsWorld::from_scene(&scene);
    let (id, toi) = physics
        .cast_ray(Vec3::ZERO, Vec3::Z, 100.0)
        .expect("should hit");
    assert_eq!(id, target, "hits the target entity");
    assert!((toi - 4.0).abs() < 0.05, "front face is at 4.0, got {toi}");
}

#[test]
fn cast_ray_misses_into_empty_space() {
    let mut scene = Scene::new();
    static_box(&mut scene, Vec3::new(0.0, 0.0, 5.0), Vec3::splat(2.0));
    let physics = PhysicsWorld::from_scene(&scene);
    // Fire away from the box: a real miss must be None, never a constant hit.
    assert!(physics.cast_ray(Vec3::ZERO, -Vec3::Z, 100.0).is_none());
}

#[test]
fn cast_ray_with_normal_reports_toi_and_facing_normal() {
    let mut scene = Scene::new();
    static_box(&mut scene, Vec3::new(0.0, 0.0, 5.0), Vec3::splat(2.0));
    let physics = PhysicsWorld::from_scene(&scene);
    let (toi, n) = physics
        .cast_ray_with_normal(Vec3::ZERO, Vec3::Z, 100.0)
        .expect("should hit");
    assert!((toi - 4.0).abs() < 0.05, "front face is at 4.0, got {toi}");
    // The struck front face points back toward the ray origin (-Z).
    assert!(n.z < -0.5, "normal should face -Z, got {n:?}");
}

#[test]
fn cast_ray_with_normal_misses_into_empty_space() {
    let mut scene = Scene::new();
    static_box(&mut scene, Vec3::new(0.0, 0.0, 5.0), Vec3::splat(2.0));
    let physics = PhysicsWorld::from_scene(&scene);
    assert!(physics
        .cast_ray_with_normal(Vec3::ZERO, -Vec3::Z, 100.0)
        .is_none());
}
