//! src/physics/recover_joint_tests.rs — what a paused start and a panic rebuild
//! keep (#1795, #1797, `recover`): a paused start leaves the joints Play built and
//! the queries see the scene; a rebuild keeps each joint's rest pose.

use glam::{Quat, Vec2, Vec3};

use super::watch::fixture_tests::{digest, fixture, scene_digest};
use crate::components::{JointComponent, JointKind};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// The fixture with a ragdoll-like limb: the dynamic body 3 on a limited ball
/// joint to the kinematic body 4, the animated bone above it.
fn limb() -> (Scene, Vec<u32>) {
    let mut scene = fixture();
    let ids = scene.world.ids().to_vec();
    let joint = JointComponent {
        kind: JointKind::Ball,
        connected_body: Some(ids[4]),
        use_limits: true,
        limits: Vec2::new(-30.0, 30.0),
        ..JointComponent::default()
    };
    scene.world.set_joint(ids[3], Some(joint));
    (scene, ids)
}

/// The limb after an Animator-like pose of its bone, with or without a paused
/// start, then three real steps: what rapier holds, and the scene.
fn posed(paused: bool) -> (String, String) {
    let (mut scene, ids) = limb();
    let mut world = PhysicsWorld::from_scene(&scene);
    if paused {
        world.step(&mut scene, 0.0);
    }
    {
        let mut bone = scene.world.transform_mut(ids[4]).unwrap();
        bone.position += Vec3::new(0.5, 0.2, 0.0);
        bone.rotation = Quat::from_rotation_z(0.6);
    }
    if paused {
        world.step(&mut scene, 0.0);
    }
    for _ in 0..3 {
        world.step(&mut scene, DT);
    }
    (digest(&world), scene_digest(&scene))
}

#[test]
fn a_paused_start_keeps_the_joints_play_built() {
    assert_eq!(posed(true), posed(false));
}

#[test]
fn a_paused_first_frame_queries_see_the_scene() {
    let mut scene = fixture();
    let ids = scene.world.ids().to_vec();
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, 0.0);
    scene.world.transform_mut(ids[4]).unwrap().position = Vec3::new(20.0, 3.0, 0.0);
    scene.world.transform_mut(ids[0]).unwrap().position = Vec3::new(-20.0, -2.0, 0.0);
    world.step(&mut scene, 0.0);
    let down = |x: f32| world.cast_ray_with_normal(Vec3::new(x, 50.0, 0.0), -Vec3::Y, 100.0);
    let kinematic = down(20.0).expect("the kinematic box where it was sent");
    assert!((kinematic.0 - 46.5).abs() < 1e-3, "{kinematic:?}");
    let slab = down(-20.0).expect("the static slab where it was moved");
    assert!((slab.0 - 51.75).abs() < 1e-3, "{slab:?}");
}

/// The rapier frames of joint `id`, and the ones it keeps.
fn frames(world: &PhysicsWorld, id: u32) -> String {
    let built = &world.joints[&id];
    let live = &world.impulse_joints.get(built.handle).unwrap().data;
    format!(
        "{:?} {:?}",
        (live.local_frame1, live.local_frame2),
        built.frames
    )
}

#[test]
fn a_rebuild_keeps_each_joints_rest_pose() {
    let (mut scene, ids) = limb();
    let mut world = PhysicsWorld::from_scene(&scene);
    world.recovery.catch = true;
    for n in 1..=40 {
        let turn = Quat::from_rotation_z(0.05 * n as f32);
        scene.world.transform_mut(ids[4]).unwrap().rotation = turn;
        world.step(&mut scene, DT);
    }
    let rest = frames(&world, ids[3]);
    let swung = frames(&PhysicsWorld::from_scene(&scene), ids[3]);
    assert_ne!(
        rest, swung,
        "the bone has turned the limb off its rest pose"
    );
    world.recovery.panic_next = true;
    assert!(world.step(&mut scene, DT).rebuilt.is_some());
    assert_eq!(frames(&world, ids[3]), rest);
}
