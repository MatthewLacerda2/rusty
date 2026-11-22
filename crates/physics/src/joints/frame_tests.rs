use glam::{Quat, Vec3};

use crate::PhysicsWorld;
use rusty_components::{
    ColliderComponent, ColliderShape, JointComponent, JointKind, RigidBodyComponent,
};
use rusty_scene::Scene;

fn frame_on_body(yaw: f32, axis: Vec3) -> Quat {
    let mut scene = Scene::new();
    let id = scene.add_entity("Cube".to_string());
    scene.world.transform_mut(id).unwrap().rotation = Quat::from_rotation_y(yaw);
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..rusty_scene::create::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        is_kinematic: false,
        ..rusty_scene::create::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    let joint = JointComponent {
        kind: JointKind::Ball,
        axis,
        ..Default::default()
    };
    scene.world.set_joint(id, Some(joint));
    let world = PhysicsWorld::from_scene(&scene);
    let frame = world.joint_world_frame(id).expect("a live joint");
    scene.world.world_rotation(id).inverse() * frame
}

fn same(a: Quat, b: Quat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-5
}

#[test]
fn a_parallel_swing_axis_falls_back_on_the_body() {
    let plain = frame_on_body(0.0, Vec3::Y);
    let yawed = frame_on_body(std::f32::consts::FRAC_PI_2, Vec3::Y);
    assert!(same(plain, yawed), "{yawed:?}, unyawed {plain:?}");
}

#[test]
fn a_zero_axis_falls_back_on_the_body() {
    let plain = frame_on_body(0.0, Vec3::ZERO);
    let yawed = frame_on_body(std::f32::consts::FRAC_PI_2, Vec3::ZERO);
    assert!(same(plain, yawed), "{yawed:?}, unyawed {plain:?}");
    assert!((plain * Vec3::X).abs_diff_eq(Vec3::X, 1e-5), "{plain:?}");
}
