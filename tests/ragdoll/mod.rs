//! Ragdolls end to end (#466): the builder's bodies and joints, the joints bending
//! the natural way, a fall that comes to rest, an impulse at the hit point, the
//! Animator ↔ physics switch, IK leaving a ragdoll alone (#461), and determinism. Saving is in-crate
//! (`scene::skeleton::ragdoll::persist_tests`), over the glTF fixture.

mod fall;
mod ik;
mod rig;
mod switch;

use glam::Vec3;
use rusty::components::JointKind;
use rusty::physics::PhysicsWorld;
use rusty::scene::skeleton::{RagdollOptions, RAGDOLL_LAYER};
use rusty::scene::Scene;

/// A scene with a floor and the rig at the origin, its ragdoll built.
pub fn ragdolled() -> (Scene, u32) {
    let mut scene = Scene::new();
    rig::floor(&mut scene);
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .expect("the rig is skinned");
    (scene, rig)
}

/// The world centre of `bone`'s hitbox.
pub fn hitbox_at(scene: &Scene, bone: u32) -> Vec3 {
    let hitbox = scene.hitbox_of(bone).expect("a hitbox bone");
    scene.compute_world_matrix(hitbox).w_axis.truncate()
}

#[test]
fn build_puts_a_body_on_every_hitbox_bone_and_joins_it_to_its_parent() {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    let made = scene
        .build_ragdoll(rig, &RagdollOptions { mass: 80.0 })
        .unwrap();
    assert_eq!(made.len(), 11, "hitboxes were generated, one body per bone");
    let bone = |n: &str| rig::bone(&scene, rig, n);
    let hips = scene.world.joint(bone("Hips")).map(|j| j.clone());
    assert!(hips.is_none(), "the root body is free");
    let knee = scene.world.joint(bone("LeftLeg")).unwrap().clone();
    assert_eq!(knee.kind, JointKind::Hinge);
    assert_eq!(knee.connected_body, Some(bone("LeftUpLeg")));
    assert!(!knee.enable_collision, "joined bones never collide");
    let arm = scene.world.joint(bone("LeftArm")).unwrap().clone();
    assert_eq!(
        (arm.kind, arm.connected_body),
        (JointKind::Ball, Some(bone("Spine")))
    );
    let mass = |n: &str| scene.world.rigidbody(bone(n)).unwrap().mass;
    let total: f32 = made
        .iter()
        .map(|&(_, b)| scene.world.rigidbody(b).unwrap().mass)
        .sum();
    assert!((total - 80.0).abs() < 1e-3, "the masses add up: {total}");
    assert!(mass("Spine") > 3.0 * mass("LeftForeArm"), "split by volume");
    assert!(made
        .iter()
        .all(|&(_, b)| scene.world.rigidbody(b).unwrap().is_kinematic));
    // The ragdoll layer collides with everything but the hitbox layer.
    let (ragdoll, hitbox) = (
        scene.layers.index_of(RAGDOLL_LAYER).unwrap(),
        scene.layers.index_of("Hitbox").unwrap(),
    );
    assert!(scene.collision_matrix.can_collide(ragdoll, 0));
    assert!(scene.collision_matrix.can_collide(ragdoll, ragdoll));
    assert!(!scene.collision_matrix.can_collide(ragdoll, hitbox));
    // Rebuilding is idempotent: same bodies, same joints.
    let shin = bone("LeftLeg");
    let again = scene
        .build_ragdoll(rig, &RagdollOptions { mass: 80.0 })
        .unwrap();
    assert_eq!(again, made);
    assert_eq!(scene.world.joint(shin).unwrap().clone(), knee);
}

/// Push `name`'s body (gravity off, everything else held) along `push` for half a
/// second, and return how far its hitbox moved along Z.
fn bend(name: &str, push: Vec3) -> f32 {
    let (mut scene, rig) = ragdolled();
    let id = rig::bone(&scene, rig, name);
    let before = hitbox_at(&scene, id);
    if let Some(mut rb) = scene.world.rigidbody_mut(id) {
        rb.is_kinematic = false;
        rb.use_gravity = false;
        rb.velocity = push;
    }
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        world.step(&mut scene, 1.0 / 60.0);
    }
    hitbox_at(&scene, id).z - before.z
}

#[test]
fn knees_bend_backward_and_elbows_forward_only() {
    assert!(bend("LeftLeg", -Vec3::Z) < -0.1, "a knee bends back");
    assert!(bend("LeftLeg", Vec3::Z).abs() < 0.03, "and not forward");
    assert!(
        bend("RightForeArm", Vec3::Z) > 0.05,
        "an elbow bends forward"
    );
    assert!(bend("RightForeArm", -Vec3::Z).abs() < 0.03, "and not back");
}
