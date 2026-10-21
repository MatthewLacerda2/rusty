//! src/physics/joints/mirrored_tests.rs — a joint under a mirroring parent
//! (#1125): an oblique hinge axis is a local direction, so the body turns about
//! the line its matrix draws (the mirror of the authored axis), not the
//! authored one.

use glam::Vec3;

use super::super::compound::world_pose;
use super::super::world::PhysicsWorld;
use crate::components::{
    ColliderComponent, ColliderShape, JointComponent, JointKind, RigidBodyComponent,
};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// A dynamic unit cube under a static parent scaled −1 on X, hinged to the
/// world about local `axis` at the cube's local −X face.
fn mirrored_hinge(axis: Vec3) -> (Scene, u32) {
    let mut scene = Scene::new();
    let mirror = scene.add_entity("Mirror".to_string());
    scene.world.transform_mut(mirror).unwrap().scale = Vec3::new(-1.0, 1.0, 1.0);
    let id = scene.add_entity("Cube".to_string());
    scene.set_parent(id, Some(mirror)).unwrap();
    scene.world.transform_mut(id).unwrap().position = Vec3::X;
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        is_kinematic: false,
        ..crate::scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    let joint = JointComponent {
        kind: JointKind::Hinge,
        anchor: -Vec3::X,
        axis,
        ..Default::default()
    };
    scene.world.set_joint(id, Some(joint));
    (scene, id)
}

#[test]
fn a_mirrored_oblique_hinge_turns_about_its_drawn_axis() {
    let axis = Vec3::new(1.0, 0.0, 1.0).normalize();
    let (mut scene, id) = mirrored_hinge(axis);
    let drawn = scene
        .compute_world_matrix(id)
        .transform_vector3(axis)
        .normalize();
    let start = world_pose(&scene, id).unwrap().rot;
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        world.step(&mut scene, DT);
    }
    let turn = world_pose(&scene, id).unwrap().rot * start.inverse();
    let (about, angle) = turn.to_axis_angle();
    assert!(angle > 0.2, "gravity swung the cube, {angle} rad");
    assert!(
        about.dot(drawn).abs() > 0.999,
        "turned about {about:?}, drawn axis {drawn:?}"
    );
}
