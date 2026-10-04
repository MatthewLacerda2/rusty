//! src/physics/joints/impulse_pair_tests.rs — the step's impulse (#806) where
//! signs matter: a weld between two free bodies ends with both moving, a kick
//! against gravity, a kinematic partner rapier never accelerates, and a steady
//! load the last substep alone reports.

use glam::Vec3;

use super::super::world::PhysicsWorld;
use super::JointBreak;
use crate::components::{ColliderComponent, ColliderShape, JointComponent, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// A one-kilogram unit cube at `pos`.
fn cube(scene: &mut Scene, pos: Vec3, use_gravity: bool, is_kinematic: bool) -> u32 {
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
        is_kinematic,
        use_gravity,
        ..crate::scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    id
}

/// `b` welded to `a`, a metre and a half to its left, at `a`'s centre.
fn weld_to(scene: &mut Scene, b: u32, a: u32, j: JointComponent) {
    let weld = JointComponent {
        connected_body: Some(a),
        anchor: Vec3::new(-1.5, 0.0, 0.0),
        ..j
    };
    scene.world.set_joint(b, Some(weld));
}

/// Set `id`'s velocities once, then step `ticks` times, collecting the breaks.
fn kick_once(scene: &mut Scene, id: u32, vel: Vec3, spin: Vec3, ticks: usize) -> Vec<JointBreak> {
    let mut world = PhysicsWorld::from_scene(scene);
    if let Some(mut rb) = scene.world.rigidbody_mut(id) {
        rb.velocity = vel;
        rb.angular_velocity = spin;
    }
    (0..ticks)
        .flat_map(|_| world.step(scene, DT).joint_breaks)
        .collect()
}

fn near(got: f32, want: f32) -> bool {
    (got - want).abs() < 0.05 * want
}

/// Two weightless cubes, B welded to A along X, with `j`'s thresholds.
fn free_pair(j: JointComponent) -> (Scene, u32) {
    let mut scene = Scene::new();
    let a = cube(&mut scene, Vec3::ZERO, false, false);
    let b = cube(&mut scene, Vec3::new(1.5, 0.0, 0.0), false, false);
    weld_to(&mut scene, b, a, j);
    (scene, b)
}

#[test]
fn a_kick_shared_by_two_free_bodies_reads_half_of_it() {
    // B kicked at 1 m/s along the weld: both end at 0.5 m/s, so the weld moved
    // half a kilogram-metre per second in one tick: 30 N.
    let j = JointComponent {
        break_force: 20.0,
        ..Default::default()
    };
    let (mut scene, b) = free_pair(j);
    let breaks = kick_once(&mut scene, b, Vec3::X, Vec3::ZERO, 3);
    assert_eq!(breaks.len(), 1, "{breaks:?}");
    assert!(near(breaks[0].force, 0.5 / DT), "{breaks:?}");
}

#[test]
fn a_spin_shared_by_two_free_bodies_reads_half_of_it() {
    // B spun at 2 rad/s about the weld's line: both end at 1 rad/s, a sixth of
    // a kilogram-metre² each, so the weld carried 10 N·m for the tick.
    let j = JointComponent {
        break_torque: 7.0,
        ..Default::default()
    };
    let (mut scene, b) = free_pair(j);
    let breaks = kick_once(&mut scene, b, Vec3::ZERO, Vec3::X * 2.0, 3);
    assert_eq!(breaks.len(), 1, "{breaks:?}");
    assert!(near(breaks[0].torque, 1.0 / 6.0 / DT), "{breaks:?}");
}

#[test]
fn a_kick_against_gravity_reads_net_of_it() {
    // Gravity already took g·dt off an upward 1 m/s kick; the weld stopped the rest.
    let mut scene = Scene::new();
    let id = cube(&mut scene, Vec3::new(0.0, 10.0, 0.0), true, false);
    let j = JointComponent {
        break_force: 40.0,
        ..Default::default()
    };
    scene.world.set_joint(id, Some(j));
    let breaks = kick_once(&mut scene, id, Vec3::Y, Vec3::ZERO, 3);
    assert_eq!(breaks.len(), 1, "{breaks:?}");
    assert!(near(breaks[0].force, (1.0 - 9.81 * DT) / DT), "{breaks:?}");
}

#[test]
fn a_kinematic_partner_adds_no_load() {
    // rapier never applies gravity to a kinematic body, so its unchanged
    // velocity is no surplus: a weightless cube welded to it carries nothing.
    let mut scene = Scene::new();
    let a = cube(&mut scene, Vec3::ZERO, true, true);
    let b = cube(&mut scene, Vec3::new(1.5, 0.0, 0.0), false, false);
    let j = JointComponent {
        break_force: 1.0,
        break_torque: 1.0,
        ..Default::default()
    };
    weld_to(&mut scene, b, a, j);
    let breaks = kick_once(&mut scene, b, Vec3::ZERO, Vec3::ZERO, 10);
    assert!(breaks.is_empty(), "{breaks:?}");
}

#[test]
fn a_steady_load_the_estimate_misses_still_breaks() {
    // B hangs off A, which hangs off the world: both held still, their velocity
    // changes match and the estimate reads nothing for B's weld. The last
    // substep reads B's weight (most of it on tick 1, before A settles).
    let mut scene = Scene::new();
    let a = cube(&mut scene, Vec3::new(0.0, 10.0, 0.0), true, false);
    let b = cube(&mut scene, Vec3::new(1.5, 10.0, 0.0), true, false);
    scene.world.set_joint(a, Some(JointComponent::default()));
    let j = JointComponent {
        break_force: 5.0,
        ..Default::default()
    };
    weld_to(&mut scene, b, a, j);
    let breaks = kick_once(&mut scene, b, Vec3::ZERO, Vec3::ZERO, 3);
    assert_eq!(breaks.len(), 1, "{breaks:?}");
    let force = breaks[0].force;
    assert!(force > 5.0 && force < 9.81 * 1.05, "{breaks:?}");
}
