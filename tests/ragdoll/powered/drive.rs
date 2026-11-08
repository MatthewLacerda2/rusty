//! Joint drives pull a simulated branch back to its animated pose after a hit;
//! without a drive it hangs limp.

use glam::Vec3;
use rusty::physics::PhysicsWorld;
use rusty::scene::skeleton::Drive;

use super::{at_rest, rig, step, turn};

/// The left arm simulated with `strength`, its forearm knocked back hard; how
/// far (radians) the forearm is off its animated turn after 3 ticks and after
/// 0.6 s.
fn knocked(strength: f32) -> (f32, f32) {
    knocked_then(strength, |_, _| {})
}

/// [`knocked`], with `between` run on the scene and forearm after the 3 ticks.
fn knocked_then(strength: f32, between: impl Fn(&mut rusty::scene::Scene, u32)) -> (f32, f32) {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    let arm = rig::bone(&scene, rig, "LeftArm");
    let fore = rig::bone(&scene, rig, "LeftForeArm");
    scene.set_simulated(rig, arm, true, &at_rest);
    let damping = 2.0 * strength.sqrt();
    assert_eq!(
        scene.set_drive(rig, arm, [Some(strength), Some(damping)]),
        2
    );
    let animated = turn(&scene, rig, "LeftForeArm");
    let mut physics = PhysicsWorld::from_scene(&scene);
    scene.world.rigidbody_mut(fore).unwrap().velocity = Vec3::new(0.0, 0.0, -3.0);
    step(&mut scene, &mut physics, 3);
    let early = turn(&scene, rig, "LeftForeArm").angle_between(animated);
    between(&mut scene, fore);
    step(&mut scene, &mut physics, 33);
    let late = turn(&scene, rig, "LeftForeArm").angle_between(animated);
    (early, late)
}

#[test]
fn a_strong_drive_yields_to_the_hit_and_returns_to_the_animated_pose() {
    let (early, late) = knocked(30000.0);
    assert!(early > 0.05, "the hit moved the forearm: {early}");
    assert!(late < 0.08, "back within ~5° in 0.6 s: {late}");
}

#[test]
fn without_a_drive_the_branch_hangs_limp() {
    let (_, late) = knocked(0.0);
    assert!(late > 0.5, "it hangs, far from the T-pose: {late}");
}

#[test]
fn the_drive_reaches_an_animated_pose_away_from_rest() {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    let arm = rig::bone(&scene, rig, "LeftArm");
    let fore = rig::bone(&scene, rig, "LeftForeArm");
    scene.set_simulated(rig, arm, true, &at_rest);
    let drive = Drive {
        strength: 2000.0,
        damping: 90.0,
    };
    scene.set_drive(rig, arm, [Some(drive.strength), Some(drive.damping)]);
    // The Animator bends the elbow forward 0.6 rad (+X toward +Z).
    let (pos, rot) = scene.ragdolls.animated(fore).unwrap();
    let bent = glam::Quat::from_rotation_y(-0.6) * rot;
    scene.ragdolls.note_animated(fore, (pos, bent));
    let mut physics = PhysicsWorld::from_scene(&scene);
    step(&mut scene, &mut physics, 60);
    let want = turn(&scene, rig, "LeftArm") * bent;
    let off = turn(&scene, rig, "LeftForeArm").angle_between(want);
    assert!(off < 0.06, "the forearm follows the animated bend: {off}");
    let rest = turn(&scene, rig, "LeftArm") * rot;
    let from_rest = turn(&scene, rig, "LeftForeArm").angle_between(rest);
    assert!(from_rest > 0.5, "and is not at rest: {from_rest}");
}
