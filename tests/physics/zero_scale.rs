//! A collider under a zero-scaled ancestor across a save and a reload:
//! glam splits a zero axis into a NaN rotation, and NaN serializes as `null`,
//! which then fails to load, so the stored AABB must stay finite; a body's
//! write-back under such a parent keeps the local position it cannot solve for
//!. The pose and shape half is `physics::degenerate_parent_tests`.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};

use rusty::components::{CapsuleAxis, ColliderComponent, ColliderShape};
use rusty::edit::{apply_scene_data, to_scene_data};
use rusty::physics::PhysicsWorld;
use rusty::scene::create::defaults::default_rigidbody;
use rusty::scene::Scene;

/// A capsule child, turned 90° about Z, under a parent scaled by `parent_scale`
/// and turned about Y. Returns the child's id.
fn child_under(scene: &mut Scene, parent_scale: Vec3) -> u32 {
    let parent = scene.add_entity("P".to_string());
    let mut t = scene.world.transform_mut(parent).unwrap();
    (t.position, t.rotation, t.scale) = (
        Vec3::new(1.0, 2.0, 3.0),
        Quat::from_rotation_y(0.6),
        parent_scale,
    );
    drop(t);
    let id = scene.add_entity("C".to_string());
    scene.set_parent(id, Some(parent)).unwrap();
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation) = (Vec3::X, Quat::from_rotation_z(FRAC_PI_2));
    drop(t);
    let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    c.shape = ColliderShape::Capsule {
        radius: 0.5,
        height: 2.0,
        axis: CapsuleAxis::Y,
    };
    scene.world.set_collider(id, Some(c));
    id
}

const DEGENERATE: [Vec3; 2] = [Vec3::new(0.0, 1.0, 1.0), Vec3::ZERO];

#[test]
fn a_scene_with_a_zero_scaled_parent_saves_and_reloads() {
    for parent_scale in DEGENERATE {
        let mut scene = Scene::new();
        let id = child_under(&mut scene, parent_scale);
        let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
        let data = serde_json::from_str(&json).expect("the saved scene loads");
        let mut restored = Scene::new();
        apply_scene_data(&mut restored, data);
        let c = restored.world.collider(id).unwrap();
        assert!(c.aabb_min.is_finite() && c.aabb_max.is_finite());
    }
}

#[test]
fn a_body_under_a_zero_scaled_group_stays_finite_through_a_step_and_a_reload() {
    let mut scene = Scene::new();
    let id = child_under(&mut scene, Vec3::new(0.0, 1.0, 1.0));
    scene.world.set_rigidbody(id, Some(default_rigidbody()));
    let mut physics = PhysicsWorld::from_scene(&scene);
    physics.step(&mut scene, 1.0 / 60.0);
    assert!(scene.world.transform(id).unwrap().position.is_finite());

    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let data = serde_json::from_str(&json).expect("the saved scene loads");
    let mut restored = Scene::new();
    apply_scene_data(&mut restored, data);
    assert!(restored.world.transform(id).unwrap().position.is_finite());
}
