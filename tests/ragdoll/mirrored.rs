//! A ragdoll under a mirroring parent (#1119): the rig scaled −1 on X, the way a
//! mirrored left/right rig is often built. When physics takes over every limb
//! starts where the Animator left it, and a body physics turns turns its bone
//! the same way. Before the fix the bone turned the mirror of the way its body
//! did, and its hitbox drifted off its own collider.

use glam::{Quat, Vec3};
use rusty::physics::PhysicsWorld;
use rusty::scene::skeleton::RagdollOptions;
use rusty::scene::Scene;

use super::rig;

/// The X mirror, as a scale.
const MIRROR: Vec3 = Vec3::new(-1.0, 1.0, 1.0);

/// The rig with a bent arm and leg (a bone with no rotation looks the same in
/// the mirror), mirrored, ragdolled with gravity off; its left forearm
/// and shin pushed along `push`, stepped for `frames`. The scene, its physics and
/// every ragdoll bone's (name, hitbox).
fn pushed(push: Vec3, frames: usize) -> (Scene, PhysicsWorld, Vec<(String, u32)>) {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    scene.world.transform_mut(rig).unwrap().scale = MIRROR;
    for (name, rot) in [
        ("LeftArm", Quat::from_rotation_z(-0.4)),
        ("LeftForeArm", Quat::from_rotation_y(0.5)),
        (
            "LeftLeg",
            Quat::from_rotation_x(0.3) * Quat::from_rotation_z(0.2),
        ),
    ] {
        let bone = rig::bone(&scene, rig, name);
        scene.world.transform_mut(bone).unwrap().rotation = rot;
    }
    let made = scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .unwrap();
    let bones: Vec<u32> = made.into_iter().map(|(_, bone)| bone).collect();
    for &bone in &bones {
        let mut rb = scene.world.rigidbody_mut(bone).unwrap();
        rb.is_kinematic = false;
        rb.use_gravity = false;
    }
    for name in ["LeftForeArm", "LeftLeg"] {
        let bone = rig::bone(&scene, rig, name);
        scene.world.rigidbody_mut(bone).unwrap().velocity = push;
    }
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..frames {
        world.step(&mut scene, 1.0 / 60.0);
    }
    let hitboxes = bones
        .iter()
        .map(|&b| {
            let name = scene.world.name(b).map(|n| n.clone()).unwrap_or_default();
            (name, scene.hitbox_of(b).unwrap())
        })
        .collect();
    (scene, world, hitboxes)
}

#[test]
fn a_mirrored_limb_turns_the_way_its_body_does() {
    let (scene, world, hitboxes) = pushed(Vec3::new(0.8, 0.5, -1.5), 30);
    for (name, hitbox) in hitboxes {
        let centre = scene.compute_world_matrix(hitbox).w_axis.truncate();
        assert_eq!(
            world.collider_contains_point(hitbox, centre),
            Some(true),
            "{name}'s hitbox at {centre} left its collider"
        );
    }
}

#[test]
fn the_first_physics_frame_keeps_a_mirrored_limbs_world_pose() {
    let at = |s: &Scene, id: u32| s.compute_world_matrix(id).w_axis.truncate();
    let (before, _, hitboxes) = pushed(Vec3::ZERO, 0);
    let (after, world, _) = pushed(Vec3::ZERO, 1);
    for (name, hitbox) in hitboxes {
        let (was, now) = (at(&before, hitbox), at(&after, hitbox));
        assert!(now.abs_diff_eq(was, 1e-4), "{name} moved: {was} → {now}");
        assert_eq!(world.collider_contains_point(hitbox, now), Some(true));
    }
}
