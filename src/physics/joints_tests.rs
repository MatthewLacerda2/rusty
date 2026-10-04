//! src/physics/joints_tests.rs — `Joint` components in the rapier world (#449): a
//! fixed joint holds and welds, a hinge swings within its limits, a ball joint is
//! a pendulum whose two swing limits stop it per axis (#675), a joint breaks past
//! its force, and a run replays identically.

use glam::{Quat, Vec2, Vec3};

use super::{JointBreak, PhysicsWorld};
use crate::components::{
    ColliderComponent, ColliderShape, JointComponent, JointKind, RigidBodyComponent,
};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// A dynamic unit cube at `pos` (density 1: one kilogram).
fn cube(scene: &mut Scene, pos: Vec3) -> u32 {
    let id = scene.add_entity("Cube".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let shape = ColliderShape::Box { size: Vec3::ONE };
    let collider = ColliderComponent {
        active: true,
        shape,
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

fn joint(scene: &mut Scene, id: u32, j: JointComponent) {
    scene.world.set_joint(id, Some(j));
}

/// Step `ticks` times, collecting every break.
fn run(scene: &mut Scene, ticks: usize) -> Vec<JointBreak> {
    let mut world = PhysicsWorld::from_scene(scene);
    (0..ticks)
        .flat_map(|_| world.step(scene, DT).joint_breaks)
        .collect()
}

fn pos(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

/// A cube one unit right of a world pivot at the origin, hinged about Z.
fn hinged(limits: Option<Vec2>) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = cube(&mut scene, Vec3::X);
    let j = JointComponent {
        kind: JointKind::Hinge,
        anchor: -Vec3::X,
        axis: Vec3::Z,
        use_limits: limits.is_some(),
        limits: limits.unwrap_or_default(),
        ..Default::default()
    };
    joint(&mut scene, id, j);
    (scene, id)
}

/// The hinged cube's swing about Z, in degrees (negative: fallen clockwise).
fn swing_deg(scene: &Scene, id: u32) -> f32 {
    let rot: Quat = scene.world.transform(id).unwrap().rotation;
    let (axis, angle) = rot.to_axis_angle();
    (angle * axis.z.signum()).to_degrees()
}

#[test]
fn fixed_joint_to_the_world_holds_a_body_up() {
    let mut scene = Scene::new();
    let id = cube(&mut scene, Vec3::new(0.0, 5.0, 0.0));
    joint(&mut scene, id, JointComponent::default());
    assert!(run(&mut scene, 120).is_empty());
    let p = pos(&scene, id);
    assert!(
        (p - Vec3::new(0.0, 5.0, 0.0)).length() < 0.05,
        "held at {p}"
    );
}

#[test]
fn fixed_joint_welds_two_bodies() {
    let mut scene = Scene::new();
    let a = cube(&mut scene, Vec3::new(0.0, 10.0, 0.0));
    let b = cube(&mut scene, Vec3::new(1.5, 10.0, 0.0));
    let weld = JointComponent {
        connected_body: Some(a),
        ..Default::default()
    };
    joint(&mut scene, b, weld);
    run(&mut scene, 40);
    let (pa, pb) = (pos(&scene, a), pos(&scene, b));
    assert!(pa.y < 8.0, "the pair falls together: {pa}");
    assert!(
        (pb - pa - Vec3::new(1.5, 0.0, 0.0)).length() < 0.05,
        "{pa} {pb}"
    );
}

#[test]
fn hinge_swings_freely_without_limits_and_stops_at_them() {
    let (mut free, id) = hinged(None);
    run(&mut free, 40);
    let p = pos(&free, id);
    assert!(
        (p.length() - 1.0).abs() < 0.05,
        "the pivot distance holds: {p}"
    );
    assert!(p.z.abs() < 0.01, "it stays in the hinge plane: {p}");
    assert!(swing_deg(&free, id) < -60.0, "{}", swing_deg(&free, id));

    let (mut limited, id) = hinged(Some(Vec2::new(-30.0, 30.0)));
    let mut world = PhysicsWorld::from_scene(&limited);
    for _ in 0..90 {
        world.step(&mut limited, DT);
        let deg = swing_deg(&limited, id);
        assert!((-31.0..=0.5).contains(&deg), "within the limit: {deg}");
    }
    assert!(swing_deg(&limited, id) < -29.0, "it rests on the limit");
}

#[test]
fn ball_joint_is_a_pendulum() {
    let mut scene = Scene::new();
    let id = cube(&mut scene, Vec3::new(1.0, 0.0, 0.3));
    let ball = JointComponent {
        kind: JointKind::Ball,
        anchor: Vec3::new(-1.0, 0.0, -0.3),
        ..Default::default()
    };
    joint(&mut scene, id, ball);
    let reach = Vec3::new(1.0, 0.0, 0.3).length();
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..40 {
        world.step(&mut scene, DT);
        let p = pos(&scene, id);
        assert!((p.length() - reach).abs() < 0.05, "on the sphere: {p}");
    }
    assert!(pos(&scene, id).y < -0.5, "it swung down");
}

#[test]
fn a_joint_breaks_past_its_force_and_not_before() {
    // The cube weighs ~9.81 N, so a 50 N joint holds and a 5 N one snaps.
    let rated = |force: f32| {
        let mut scene = Scene::new();
        let id = cube(&mut scene, Vec3::new(0.0, 5.0, 0.0));
        let j = JointComponent {
            break_force: force,
            ..Default::default()
        };
        joint(&mut scene, id, j);
        let breaks = run(&mut scene, 60);
        (scene, id, breaks)
    };
    let (strong, id, breaks) = rated(50.0);
    assert!(breaks.is_empty() && strong.world.has_joint(id));

    let (weak, id, breaks) = rated(5.0);
    assert_eq!(breaks.len(), 1, "breaks once: {breaks:?}");
    assert_eq!(breaks[0].id, id);
    // The load it broke under is the cube's weight, in newtons.
    assert!((breaks[0].force - 9.81).abs() < 0.05, "{breaks:?}");
    assert!(!weak.world.has_joint(id), "a broken joint is destroyed");
    assert!(pos(&weak, id).y < 4.5, "and the cube falls");
}

#[test]
fn joints_replay_identically() {
    let once = || {
        let (mut scene, id) = hinged(Some(Vec2::new(-60.0, 10.0)));
        run(&mut scene, 90);
        let t = scene.world.transform(id).unwrap().clone();
        (t.position, t.rotation)
    };
    assert_eq!(once(), once());
}

#[test]
fn play_mode_edits_rebuild_only_what_changed() {
    let mut scene = Scene::new();
    let id = cube(&mut scene, Vec3::new(0.0, 5.0, 0.0));
    joint(&mut scene, id, JointComponent::default());
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    let handle = world.joints[&id].handle;
    scene.world.joint_mut(id).unwrap().break_force = 500.0;
    world.step(&mut scene, DT);
    assert_eq!(world.joints[&id].handle, handle, "a threshold is read live");
    scene.world.joint_mut(id).unwrap().kind = JointKind::Ball;
    world.step(&mut scene, DT);
    assert_ne!(world.joints[&id].handle, handle, "a new kind rebuilds");
    scene.world.set_joint(id, None);
    for _ in 0..30 {
        world.step(&mut scene, DT);
    }
    assert!(world.joints.is_empty() && world.impulse_joints.is_empty());
    assert!(pos(&scene, id).y < 4.5, "a removed joint frees the body");
}

/// A cube one unit right of a world pivot, on a limited Ball joint with the given
/// swing axis and swing 1 / swing 2 limits; after `ticks`, how far (degrees) it
/// has fallen below the horizontal, with the deepest it ever got.
fn ball_fall(swing_axis: Vec3, swing1: f32, swing2: f32, ticks: usize) -> (f32, f32) {
    let mut scene = Scene::new();
    let id = cube(&mut scene, Vec3::X);
    let ball = JointComponent {
        kind: JointKind::Ball,
        anchor: -Vec3::X,
        swing_axis,
        use_limits: true,
        swing_limit: swing1,
        swing2_limit: swing2,
        ..Default::default()
    };
    joint(&mut scene, id, ball);
    let mut world = PhysicsWorld::from_scene(&scene);
    let mut deepest: f32 = 0.0;
    let mut fall = 0.0;
    for _ in 0..ticks {
        world.step(&mut scene, DT);
        let p = pos(&scene, id);
        fall = (-p.y).atan2(p.x).to_degrees();
        deepest = deepest.max(fall);
    }
    (fall, deepest)
}

#[test]
fn ball_joint_stops_at_each_axis_own_swing_limit() {
    // Gravity swings the cube about Z: swing 2 with a Y swing axis, swing 1 with Z.
    for (axis, s1, s2, limit) in [
        (Vec3::Y, 60.0, 20.0, 20.0),
        (Vec3::Y, 20.0, 60.0, 60.0),
        (Vec3::Z, 20.0, 60.0, 20.0),
        (Vec3::Z, 60.0, 20.0, 60.0),
    ] {
        let (fall, deepest) = ball_fall(axis, s1, s2, 120);
        let case = format!("axis {axis}, swing {s1}/{s2}: fell {fall}, deepest {deepest}");
        assert!(deepest < limit + 2.0, "never past the limit — {case}");
        assert!(fall > limit - 2.0, "rests on the limit — {case}");
    }
}

#[test]
fn ball_swing_limits_replay_identically() {
    let once = || ball_fall(Vec3::new(0.0, 1.0, 1.0), 15.0, 50.0, 90);
    assert_eq!(once(), once());
}
