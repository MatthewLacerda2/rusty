//! src/physics/write_back_tests.rs — a solved body's write-back moves the colliders
//! hanging under it (#1157): the Transform write re-derives the owner's subtree.

use glam::Vec3;

use super::PhysicsWorld;
use crate::scene::authoring::defaults::{default_collider, default_rigidbody};
use crate::scene::Scene;

#[test]
fn a_falling_body_drags_its_childs_bounds_down_with_it() {
    let mut scene = Scene::new();
    let body = scene.add_entity("body".into());
    scene.world.transform_mut(body).unwrap().position = Vec3::new(0.0, 10.0, 0.0);
    scene.world.set_rigidbody(body, Some(default_rigidbody()));
    let child = scene.add_entity("child".into());
    scene.set_parent(child, Some(body)).unwrap();
    scene.world.transform_mut(child).unwrap().position = Vec3::X * 2.0;
    for id in [body, child] {
        scene.world.set_collider(id, Some(default_collider()));
        scene.update_entity_collider(id);
    }
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..10 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    let fell = scene.world.world_position(child);
    assert!(fell.y < 10.0, "it fell: {fell}");
    let c = scene.world.collider(child).unwrap();
    let centre = (c.aabb_min + c.aabb_max) * 0.5;
    assert!(centre.abs_diff_eq(fell, 1e-4), "{centre} vs {fell}");
}
