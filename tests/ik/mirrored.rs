//! IK under a mirroring parent (#1119): a rig scaled −1 on X, the way a mirrored
//! left/right rig is often built, reaches the mirror of [`super::TARGET`] with
//! its mirrored arm, not the mirror of where it aimed; an aim chain points its
//! drawn axis at its target (#1125).

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

/// A look chain on a mirrored rig aiming the head's local `axis` at `target`.
fn look_with(axis: &str, target: Vec3) -> String {
    format!(
        "return {{ Start = function(id)\n\
  Animator.AddAimIK(id, 'Look', {{ 'pelvis', 'spine', 'head' }}, {{ axis = {axis} }})\n\
  Animator.SetIKTarget(id, 'Look', {}, {}, {})\nend }}",
        target.x, target.y, target.z
    )
}

/// The head's drawn `axis` (through its world matrix, mirror and all) meets the
/// target: an X axis no longer aims 180° away, a diagonal not at its mirror (#1125).
#[test]
fn an_aim_axis_under_a_mirroring_parent_points_at_the_target() {
    let target = Vec3::new(-3.0, 1.85, 2.0);
    for (lua, axis) in [
        ("{1, 0, 0}", Vec3::X),
        ("{1, 0, 1}", Vec3::new(1.0, 0.0, 1.0)),
    ] {
        let (scene, rig) = rig_with(&look_with(lua, target), |s, id| {
            s.world.transform_mut(id).unwrap().scale = Vec3::new(-1.0, 1.0, 1.0);
        });
        let gw = play(scene, 5);
        let s = gw.scene().borrow();
        let head = s.find_bone(rig, "head").unwrap();
        let drawn = s.compute_world_matrix(head).transform_vector3(axis);
        let want = target - bone_at(&s, rig, "head");
        let off = drawn.angle_between(want).to_degrees();
        assert!(off < 0.1, "axis {lua} aims {off}° off the target");
    }
}
