use glam::Vec3;

use super::super::world::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape, JointComponent, RigidBodyComponent};
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn cube(scene: &mut Scene, pos: Vec3, dynamic: bool) -> u32 {
    let id = scene.add_entity("Cube".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..rusty_scene::create::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    if dynamic {
        let rb = RigidBodyComponent {
            is_kinematic: false,
            ..rusty_scene::create::defaults::default_rigidbody()
        };
        scene.world.set_rigidbody(id, Some(rb));
    }
    id
}

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
