//! src/physics/joints/load_tests.rs — the torque a joint carries about its anchor
//! (#803): a weight welded off its pivot loads it with weight × lever (Unity's
//! `currentTorque`), a free hinge axis or a ball carries none, a weld between two
//! moving bodies reads the same as one to the world, and `break_torque` breaks on it.

use glam::Vec3;

use super::super::world::PhysicsWorld;
use super::load::anchor_torque;
use crate::components::{
    ColliderComponent, ColliderShape, JointComponent, JointKind, RigidBodyComponent,
};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;
/// A unit cube's weight: one kilogram under the default gravity.
const WEIGHT: f32 = 9.81;

/// A dynamic unit cube (one kilogram) at `pos`.
fn cube(scene: &mut Scene, pos: Vec3) -> u32 {
    let id = scene.add_entity("Cube".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size: Vec3::ONE },
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        is_kinematic: false,
        ..crate::scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    id
}

/// A cube `lever` metres right of its joint's pivot, far from anything else.
fn off_pivot(kind: JointKind, axis: Vec3, lever: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = cube(&mut scene, Vec3::new(lever, 10.0, 0.0));
    let j = JointComponent {
        kind,
        anchor: -Vec3::X * lever,
        axis,
        ..Default::default()
    };
    scene.world.set_joint(id, Some(j));
    (scene, id)
}

/// The torque (N·m) joint `id` carries after `ticks` steps.
fn torque_after(scene: &mut Scene, id: u32, ticks: usize) -> f32 {
    let mut world = PhysicsWorld::from_scene(scene);
    for _ in 0..ticks {
        assert!(world.step(scene, DT).joint_breaks.is_empty());
    }
    let substep = DT / world.integration_parameters.num_solver_iterations as f32;
    let live = world.impulse_joints.get(world.joints[&id].handle).unwrap();
    anchor_torque(live, &world.bodies).length() / substep
}

fn near(got: f32, want: f32) -> bool {
    (got - want).abs() < 0.02 * want.max(1.0)
}

#[test]
fn a_weld_off_its_pivot_carries_weight_times_lever() {
    for lever in [1.0, 2.0] {
        let (mut scene, id) = off_pivot(JointKind::Fixed, Vec3::X, lever);
        let t = torque_after(&mut scene, id, 30);
        assert!(near(t, WEIGHT * lever), "lever {lever} m: {t} N·m");
    }
    let (mut scene, id) = off_pivot(JointKind::Fixed, Vec3::X, 0.0);
    let t = torque_after(&mut scene, id, 30);
    assert!(t < 0.01, "welded at its centre, no lever: {t} N·m");
}

#[test]
fn only_locked_rotation_carries_torque() {
    // Gravity twists the cube about Z: a hinge about Y holds that, one about Z
    // swings freely, and a ball joint holds no rotation at all.
    let (mut held, id) = off_pivot(JointKind::Hinge, Vec3::Y, 1.0);
    let t = torque_after(&mut held, id, 1);
    assert!(near(t, WEIGHT), "hinge across the twist: {t}");
    for (kind, axis) in [(JointKind::Hinge, Vec3::Z), (JointKind::Ball, Vec3::X)] {
        let (mut free, id) = off_pivot(kind, axis, 1.0);
        let t = torque_after(&mut free, id, 1);
        assert!(t < 0.01, "{kind:?} about {axis}: {t}");
    }
}

#[test]
fn a_weld_between_two_moving_bodies_reads_the_same() {
    // A held to the world; B welded to A a metre and a half to its right. By tick
    // 60 both sleep, on the impulses of their last awake step.
    let mut scene = Scene::new();
    let a = cube(&mut scene, Vec3::new(0.0, 10.0, 0.0));
    let b = cube(&mut scene, Vec3::new(1.5, 10.0, 0.0));
    scene.world.set_joint(a, Some(JointComponent::default()));
    let weld = JointComponent {
        connected_body: Some(a),
        anchor: Vec3::new(-1.5, 0.0, 0.0),
        ..Default::default()
    };
    scene.world.set_joint(b, Some(weld));
    let t = torque_after(&mut scene, b, 60);
    assert!(near(t, WEIGHT * 1.5), "{t} N·m");
}

#[test]
fn a_weld_breaks_past_its_torque_and_not_before() {
    let rated = |break_torque: f32| {
        let (mut scene, id) = off_pivot(JointKind::Fixed, Vec3::X, 1.0);
        scene.world.joint_mut(id).unwrap().break_torque = break_torque;
        let mut world = PhysicsWorld::from_scene(&scene);
        let breaks: Vec<_> = (0..60)
            .flat_map(|_| world.step(&mut scene, DT).joint_breaks)
            .collect();
        (scene, id, breaks)
    };
    let (strong, id, breaks) = rated(15.0);
    assert!(
        breaks.is_empty() && strong.world.has_joint(id),
        "{breaks:?}"
    );

    let (weak, id, breaks) = rated(5.0);
    assert_eq!(breaks.len(), 1, "breaks once: {breaks:?}");
    assert!(near(breaks[0].torque, WEIGHT), "{breaks:?}");
    assert!(near(breaks[0].force, WEIGHT), "the force reads true too");
    assert!(!weak.world.has_joint(id), "a broken joint is destroyed");
}
