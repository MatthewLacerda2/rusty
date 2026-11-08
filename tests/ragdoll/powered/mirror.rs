//! The drive under a mirroring rig (#1119, #1895): FBX's `(1,1,−1)` scale and a
//! −1 X mirror. The engine draws a bone under a mirror as `F·R_local·F`, so the
//! drive's target is the animated turn the bone is drawn with, not the plain
//! product of local rotations.

use glam::{Quat, Vec3};
use rusty::physics::PhysicsWorld;
use rusty::scene::skeleton::{Drive, RagdollOptions};
use rusty::scene::Scene;

use super::{at_rest, rig, step, turn};

/// How far (radians) the `scale`'d rig's driven forearm ends from the world
/// rotation its animated bend is drawn at.
fn off_the_bend(scale: Vec3) -> f32 {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    scene.world.transform_mut(rig).unwrap().scale = scale;
    scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .unwrap();
    let arm = rig::bone(&scene, rig, "LeftArm");
    let fore = rig::bone(&scene, rig, "LeftForeArm");
    scene.set_simulated(rig, arm, true, &at_rest);
    let drive = Drive {
        strength: 2000.0,
        damping: 90.0,
    };
    scene.set_drive(rig, arm, [Some(drive.strength), Some(drive.damping)]);
    // The elbow bent forward: under a mirror its drawn turn is not the plain one.
    let (pos, rot) = scene.ragdolls.animated(fore).unwrap();
    let bent = Quat::from_rotation_y(-0.6) * rot;
    scene.ragdolls.note_animated(fore, (pos, bent));
    let mut physics = PhysicsWorld::from_scene(&scene);
    step(&mut scene, &mut physics, 60);
    let got = turn(&scene, rig, "LeftForeArm");
    // Where the bend is drawn under the arm physics left: the bone posed so.
    let held = scene.world.transform(fore).unwrap().rotation;
    scene.world.transform_mut(fore).unwrap().rotation = bent;
    let want = turn(&scene, rig, "LeftForeArm");
    scene.world.transform_mut(fore).unwrap().rotation = held;
    got.angle_between(want)
}

#[test]
fn the_drive_reaches_the_drawn_bend_unmirrored_and_mirrored() {
    for scale in [
        Vec3::ONE,
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(-1.0, 1.0, 1.0),
    ] {
        let off = off_the_bend(scale);
        assert!(off < 0.06, "under {scale}: {off} rad off the bend");
    }
}
