//! src/physics/joints/inert_tests.rs — which `Joint`s get built (#449): one
//! missing a body, or naming its own body as the connected one, is inert, not an
//! error; and an explicit `connected_anchor` is read in the connected body's
//! local space.

use glam::Vec3;

use super::super::world::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape, JointComponent, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// A unit cube at `pos`: dynamic with a Rigidbody, else a static collider.
fn cube(scene: &mut Scene, pos: Vec3, dynamic: bool) -> u32 {
    let id = scene.add_entity("Cube".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    if dynamic {
        let rb = RigidBodyComponent {
            is_kinematic: false,
            ..crate::scene::authoring::defaults::default_rigidbody()
        };
        scene.world.set_rigidbody(id, Some(rb));
    }
    id
}

/// Height of `id` after a second of simulation.
fn y_after_a_second(scene: &mut Scene, id: u32) -> f32 {
    let mut world = PhysicsWorld::from_scene(scene);
    for _ in 0..60 {
        world.step(scene, DT);
    }
    scene.world.transform(id).unwrap().position.y
}

#[test]
fn a_joint_without_a_buildable_pair_is_inert_and_its_body_falls() {
    let mut scene = Scene::new();
    let marker = scene.add_entity("Marker".to_string());
    let to_bodiless = cube(&mut scene, Vec3::new(0.0, 10.0, 0.0), true);
    let to_itself = cube(&mut scene, Vec3::new(5.0, 10.0, 0.0), true);
    let bodiless = scene.add_entity("Bodiless".to_string());
    for (id, other) in [(to_bodiless, Some(marker)), (to_itself, Some(to_itself))] {
        let j = JointComponent {
            connected_body: other,
            ..Default::default()
        };
        scene.world.set_joint(id, Some(j));
    }
    // A joint on an entity with no body at all is skipped, never a panic.
    scene
        .world
        .set_joint(bodiless, Some(JointComponent::default()));
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..60 {
        world.step(&mut scene, DT);
    }
    for id in [to_bodiless, to_itself] {
        let y = scene.world.transform(id).unwrap().position.y;
        assert!(y < 6.0, "entity {id} fell freely: y = {y}");
    }
}

#[test]
fn an_explicit_connected_anchor_is_in_the_connected_body_s_space() {
    // The pivot is the top face of a static cube at y = 5 and the bottom face of
    // a dynamic one at y = 6: the frames agree, so the weld holds it in place.
    let mut scene = Scene::new();
    let base = cube(&mut scene, Vec3::new(0.0, 5.0, 0.0), false);
    let top = cube(&mut scene, Vec3::new(0.0, 6.0, 0.0), true);
    let j = JointComponent {
        connected_body: Some(base),
        anchor: Vec3::new(0.0, -0.5, 0.0),
        connected_anchor: Vec3::new(0.0, 0.5, 0.0),
        auto_configure_connected_anchor: false,
        ..Default::default()
    };
    scene.world.set_joint(top, Some(j.clone()));
    let held = y_after_a_second(&mut scene, top);
    assert!((held - 6.0).abs() < 0.05, "held at its pivot: y = {held}");
    // The same anchor read as world space would sit 4.5 m lower: the body is
    // pulled to it, which shows the anchor is taken in the base's frame.
    let mut scene = Scene::new();
    let top = cube(&mut scene, Vec3::new(0.0, 6.0, 0.0), true);
    let world_frame = JointComponent {
        connected_body: None,
        ..j
    };
    scene.world.set_joint(top, Some(world_frame));
    let pulled = y_after_a_second(&mut scene, top);
    assert!(
        pulled < 2.0,
        "pulled to the world-space pivot: y = {pulled}"
    );
}
