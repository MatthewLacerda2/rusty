//! The aim chain through the tick, weight 0, determinism (#461).

use glam::Vec3;

use super::{bone_at, play, rig_with, REACH, TARGET};

/// `Start` gives the rig a clamped look chain toward a point 90° to its left.
const LOOK: &str = "return { Start = function(id)\n\
  Animator.AddAimIK(id, 'Look', { 'pelvis', 'spine', 'head' }, { clamp = 30 })\n\
  Animator.SetIKTarget(id, 'Look', -5, 1.85, 0)\nend }";

#[test]
fn an_aim_chain_turns_to_its_clamp_and_stays_there() {
    let (scene, rig) = rig_with(LOOK, |_, _| {});
    let gw = play(scene, 60);
    let s = gw.scene().borrow();
    let head = s.find_bone(rig, "head").unwrap();
    let (_, rot, _) = s.compute_world_matrix(head).to_scale_rotation_translation();
    let aim = rot * Vec3::Z;
    let turned = aim.angle_between(Vec3::Z).to_degrees();
    // No clip keys these bones: without IK undoing its last write first, each
    // tick would turn another 30°.
    assert!(
        (turned - 30.0).abs() < 1e-2,
        "turned {turned}° after 60 ticks"
    );
    assert!(aim.x < 0.0, "toward the target");
}

#[test]
fn weight_zero_leaves_the_pose_bit_identical() {
    let zero = REACH.replace("end }", "Animator.SetIKWeight(id, 'Arm', 0)\nend }");
    let palette = |script: &str| {
        let (scene, rig) = rig_with(script, |_, _| {});
        let gw = play(scene, 5);
        let s = gw.scene().borrow();
        assert!(bone_at(&s, rig, "finger").distance(TARGET) > 0.1);
        let p = s.world.mesh(rig).unwrap().pose_palette.clone();
        p.iter().map(|m| m.to_cols_array()).collect::<Vec<_>>()
    };
    assert_eq!(palette(&zero), palette("return {}"));
}

#[test]
fn ik_is_deterministic_across_runs() {
    let both = REACH.replace(
        "end }",
        "Animator.AddAimIK(id, 'Look', { 'pelvis', 'head' }, { clamp = 45 })\n\
         Animator.SetIKTarget(id, 'Look', -5, 1.85, 2)\nend }",
    );
    let run = || {
        let (scene, rig) = rig_with(&both, |_, _| {});
        let gw = play(scene, 30);
        let s = gw.scene().borrow();
        let p = s.world.mesh(rig).unwrap().pose_palette.clone();
        p.iter().map(|m| m.to_cols_array()).collect::<Vec<_>>()
    };
    assert_eq!(run(), run(), "same inputs must produce identical bytes");
}
