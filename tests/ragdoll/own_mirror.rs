//! A rig whose own scale mirrors it, `(1,1,−1)` as FBX import keeps (#1110):
//! its local +Z is drawn at world −Z, so that is where its front is. A knee
//! bends backward, away from the drawn front; it once bent toward it (#1145).

use glam::Vec3;
use rusty::physics::PhysicsWorld;
use rusty::scene::skeleton::RagdollOptions;
use rusty::scene::Scene;

use super::rig;

/// The world direction `scale`'d rig's left knee bends toward, as physics built it.
fn knee_bend(scale: Vec3) -> Vec3 {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    scene.world.transform_mut(rig).unwrap().scale = scale;
    scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .unwrap();
    let knee = rig::bone(&scene, rig, "LeftLeg");
    let world = PhysicsWorld::from_scene(&scene);
    world.joint_world_frame(knee).unwrap() * Vec3::Y
}

#[test]
fn a_knee_bends_away_from_the_drawn_front_under_the_rigs_own_mirror() {
    let plain = knee_bend(Vec3::ONE);
    assert!(plain.abs_diff_eq(-Vec3::Z, 1e-4), "unmirrored: {plain}");
    let mirrored = knee_bend(Vec3::new(1.0, 1.0, -1.0));
    assert!(mirrored.abs_diff_eq(Vec3::Z, 1e-4), "mirrored: {mirrored}");
}
