//! src/physics/joints/impulse_tests.rs — a joint's load is the impulse over the
//! whole step (#806): a one-off spin or kick the joint absorbs in rapier's first
//! solver substep counts against `break_torque` / `break_force`, as Unity's
//! `currentTorque` / `currentForce` do. The steady loads stay in `load_tests.rs`.

use glam::Vec3;

use super::super::world::PhysicsWorld;
use super::load::step_load;
use super::JointBreak;
use crate::components::{
    ColliderComponent, ColliderShape, JointComponent, JointKind, RigidBodyComponent,
};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;
/// A weld stopping 2 rad/s on a unit cube (inertia 1/6 kg·m²) in one tick.
const SPIN_TORQUE: f32 = 2.0 / 6.0 / DT;
/// A weld stopping 1 m/s on a one-kilogram cube in one tick.
const KICK_FORCE: f32 = 1.0 / DT;

/// A weightless one-kilogram unit cube welded to the world at its centre, with
/// `break_force` / `break_torque` set.
fn welded(break_force: f32, break_torque: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Cube".to_string());
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
        use_gravity: false,
        ..crate::scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    let j = JointComponent {
        kind: JointKind::Fixed,
        break_force,
        break_torque,
        ..Default::default()
    };
    scene.world.set_joint(id, Some(j));
    (scene, id)
}

/// Step ten ticks, setting the cube's velocities before each (a script's
/// `SetVelocity` / `SetAngularVelocity`, or a blast), collecting the breaks.
fn kicked(scene: &mut Scene, id: u32, vel: Vec3, spin: Vec3) -> Vec<JointBreak> {
    let mut world = PhysicsWorld::from_scene(scene);
    let mut breaks = Vec::new();
    for _ in 0..10 {
        if let Some(mut rb) = scene.world.rigidbody_mut(id) {
            rb.velocity = vel;
            rb.angular_velocity = spin;
        }
        breaks.extend(world.step(scene, DT).joint_breaks);
    }
    breaks
}

fn near(got: f32, want: f32) -> bool {
    (got - want).abs() < 0.05 * want
}

#[test]
fn a_spin_the_weld_stops_counts_against_break_torque() {
    let spin = Vec3::Y * 2.0;
    let (mut scene, id) = welded(0.0, SPIN_TORQUE * 1.2);
    let breaks = kicked(&mut scene, id, Vec3::ZERO, spin);
    assert!(
        breaks.is_empty(),
        "rated above the stopping torque: {breaks:?}"
    );
    assert!(scene.world.has_joint(id));

    let (mut scene, id) = welded(0.0, SPIN_TORQUE * 0.8);
    let breaks = kicked(&mut scene, id, Vec3::ZERO, spin);
    assert_eq!(breaks.len(), 1, "breaks on the first spin: {breaks:?}");
    assert!(near(breaks[0].torque, SPIN_TORQUE), "{breaks:?}");
    assert!(
        breaks[0].force < 0.01 * KICK_FORCE,
        "a pure spin: {breaks:?}"
    );
    assert!(!scene.world.has_joint(id));
}

#[test]
fn a_kick_the_weld_stops_counts_against_break_force() {
    let vel = Vec3::X;
    let (mut scene, id) = welded(KICK_FORCE * 1.2, 0.0);
    let breaks = kicked(&mut scene, id, vel, Vec3::ZERO);
    assert!(
        breaks.is_empty(),
        "rated above the stopping force: {breaks:?}"
    );

    let (mut scene, id) = welded(KICK_FORCE * 0.8, 0.0);
    let breaks = kicked(&mut scene, id, vel, Vec3::ZERO);
    assert_eq!(breaks.len(), 1, "breaks on the first kick: {breaks:?}");
    assert!(near(breaks[0].force, KICK_FORCE), "{breaks:?}");
    assert!(
        breaks[0].torque < 0.01 * SPIN_TORQUE,
        "through the anchor: {breaks:?}"
    );
}

#[test]
fn the_step_estimate_reads_a_steady_weld_true() {
    // A one-kilogram weight welded a metre right of its pivot, under gravity: the
    // estimate agrees with the last substep's reading, 9.81 N and 9.81 N·m, while
    // it is awake (asleep, the estimate reads nothing and that reading carries it).
    let (mut scene, id) = welded(0.0, 0.0);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(1.0, 10.0, 0.0);
    scene.world.rigidbody_mut(id).unwrap().use_gravity = true;
    scene.world.joint_mut(id).unwrap().anchor = -Vec3::X;
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..5 {
        world.step(&mut scene, DT);
    }
    let pre = world.snapshot_velocities();
    world.step(&mut scene, DT);
    let live = world.impulse_joints.get(world.joints[&id].handle).unwrap();
    let (lin, ang) = step_load(live, &world.bodies, &pre, world.gravity, DT);
    assert!(near(lin.length() / DT, 9.81), "force {lin}");
    assert!(near(ang.length() / DT, 9.81), "torque {ang}");
}
