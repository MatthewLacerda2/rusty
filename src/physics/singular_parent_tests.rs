//! src/physics/singular_parent_tests.rs — a dynamic body under a zero-scaled group
//! (#1164). The parent's matrix has no inverse, so the write-back keeps the local
//! position it cannot solve for instead of writing a NaN the scene cannot reload.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::{apply_scene_data, to_scene_data, Scene};

#[test]
fn a_body_under_a_zero_scaled_group_stays_finite_through_a_step_and_a_reload() {
    let mut scene = Scene::new();
    let group = scene.add_entity("group".into());
    scene.world.transform_mut(group).unwrap().scale = Vec3::new(0.0, 1.0, 1.0);
    let body = scene.add_entity("body".into());
    scene.set_parent(body, Some(group)).unwrap();
    scene.world.transform_mut(body).unwrap().position = Vec3::new(1.0, 5.0, 2.0);
    let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    c.shape = ColliderShape::Sphere { radius: 0.5 };
    scene.world.set_collider(body, Some(c));
    let rb = crate::scene::authoring::defaults::default_rigidbody();
    scene.world.set_rigidbody(body, Some(rb));
    scene.update_entity_collider(body);

    let mut physics = PhysicsWorld::from_scene(&scene);
    physics.step(&mut scene, 1.0 / 60.0);
    let local = scene.world.transform(body).unwrap().position;
    assert!(local.is_finite(), "{local}");

    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let data = serde_json::from_str(&json).expect("the saved scene loads");
    let mut restored = Scene::new();
    apply_scene_data(&mut restored, data);
    assert!(restored.world.transform(body).unwrap().position.is_finite());
}
