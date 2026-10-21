//! A ragdoll under a mirroring parent (#1119): the rig scaled −1 on X, the way a
//! mirrored left/right rig is often built. When physics takes over every limb
//! starts where the Animator left it, and a body physics turns turns its bone
//! the same way. Before the fix the bone turned the mirror of the way its body
//! did, and its hitbox drifted off its own collider.

use glam::{Quat, Vec3};
use rusty::components::transform;
use rusty::physics::PhysicsWorld;
use rusty::scene::skeleton::RagdollOptions;
use rusty::scene::Scene;

use super::rig;

/// The X mirror, as a scale.
const MIRROR: Vec3 = Vec3::new(-1.0, 1.0, 1.0);

/// The rig with a bent arm and leg (a bone with no rotation looks the same in
/// the mirror), scaled `scale`, ragdolled: the scene and its ragdoll bones.
fn ragdolled(scale: Vec3) -> (Scene, Vec<u32>) {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    scene.world.transform_mut(rig).unwrap().scale = scale;
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
    (scene, made.into_iter().map(|(_, bone)| bone).collect())
}

/// [`ragdolled`] mirrored, with gravity off; its left forearm and shin pushed
/// along `push`, stepped for `frames`. The scene, its physics and every ragdoll
/// bone's (name, hitbox).
fn pushed(push: Vec3, frames: usize) -> (Scene, PhysicsWorld, Vec<(String, u32)>) {
    let (mut scene, bones) = ragdolled(MIRROR);
    let rig = scene.find_entity_by_name("Rig").unwrap();
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

/// The world X axis physics builds `bone`'s joint frame on: its local axes seen
/// through the bone's mirror, turned by its world rotation (`build_joint`).
fn joint_world_x(scene: &Scene, bone: u32) -> Vec3 {
    let parent = scene.world.parent_world_matrix(bone);
    let t = scene.world.transform(bone).unwrap().clone();
    let j = scene.world.joint(bone).unwrap().clone();
    let mirror = transform::mirror(transform::world_scale(parent, t.scale));
    let (axis, _) = transform::mirrored_frame_axes(mirror, j.axis, j.swing_axis);
    (transform::world_rotation(parent, t.rotation) * axis).normalize()
}

/// Each mirrored joint's world X is `dir × bend` on the mirrored bones (#1125):
/// the bone direction reflects and the bend (forward, +Z) doesn't, so it is the
/// unmirrored joint's X seen as a pseudovector, `−F·x`. Before the fix, the axes
/// came back reflected and every knee flexed about the wrong sense.
#[test]
fn a_mirrored_ragdolls_joint_axes_are_the_mirror_image() {
    let (plain, plain_bones) = ragdolled(Vec3::ONE);
    let (mirrored, bones) = ragdolled(MIRROR);
    let mut joints = 0;
    for (&p, &m) in plain_bones.iter().zip(&bones) {
        if plain.world.joint(p).is_none() {
            continue;
        }
        joints += 1;
        let want = -(joint_world_x(&plain, p) * MIRROR);
        let got = joint_world_x(&mirrored, m);
        let name = mirrored
            .world
            .name(m)
            .map(|n| n.clone())
            .unwrap_or_default();
        assert!(got.abs_diff_eq(want, 1e-4), "{name}: X {got}, want {want}");
    }
    assert!(joints > 5, "every limb is jointed: {joints}");
}
