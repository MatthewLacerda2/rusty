//! IK under a mirroring parent (#1119): a rig scaled −1 on X, the way a mirrored
//! left/right rig is often built, reaches the mirror of [`super::TARGET`] with
//! its mirrored arm, not the mirror of where it aimed.

use glam::Vec3;

use super::{bone_at, play, rig_with};

/// [`super::REACH`] mirrored across X: target and hint both.
const REACH_MIRRORED: &str = "return { Start = function(id)\n\
  Animator.AddTwoBoneIK(id, 'Arm', 'spine', 'arm', 'finger')\n\
  Animator.SetIKTarget(id, 'Arm', -0.3, 1.7, 0.3)\n\
  Animator.SetIKHint(id, 'Arm', -0.3, 3, 0)\nend }";

#[test]
fn a_target_under_a_mirroring_parent_is_reached() {
    let (scene, rig) = rig_with(REACH_MIRRORED, |s, id| {
        s.world.transform_mut(id).unwrap().scale = Vec3::new(-1.0, 1.0, 1.0);
    });
    let gw = play(scene, 5);
    let s = gw.scene().borrow();
    let target = super::TARGET * Vec3::new(-1.0, 1.0, 1.0);
    let finger = bone_at(&s, rig, "finger");
    assert!(finger.abs_diff_eq(target, 1e-4), "finger at {finger:?}");
    assert!(
        bone_at(&s, rig, "arm").y > 1.5,
        "the elbow bent toward the hint"
    );
}
