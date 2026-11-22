//! IK under a mirroring parent (rig scaled −1 on X): the arm reaches the mirror of [`super::TARGET`], and an aim chain points its drawn axis at the target, a tip mirrored by its own scale too.

use glam::Vec3;

use super::{bone_at, play, rig_with};

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

fn look_with(axis: &str, target: Vec3) -> String {
    format!(
        "return {{ Start = function(id)\n\
  Animator.AddAimIK(id, 'Look', {{ 'pelvis', 'spine', 'head' }}, {{ axis = {axis} }})\n\
  Animator.SetIKTarget(id, 'Look', {}, {}, {})\nend }}",
        target.x, target.y, target.z
    )
}

/// The head's drawn `axis` (through its world matrix, mirror and all) meets the target.
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
        let drawn = s.world.local_to_world(head).transform_vector3(axis);
        let want = target - bone_at(&s, rig, "head");
        let off = drawn.angle_between(want).to_degrees();
        assert!(off < 0.1, "axis {lua} aims {off}° off the target");
    }
}

/// A tip with its own negative scale (FBX keeps one): its `{0, 0, 1}` axis is drawn along −Z of its world rotation, and still points at the target.
#[test]
fn an_aim_tip_with_its_own_mirror_points_at_the_target() {
    let target = Vec3::new(-3.0, 1.85, 2.0);
    let (scene, rig) = rig_with(&look_with("{0, 0, 1}", target), |s, id| {
        let head = s.find_bone(id, "head").unwrap();
        s.world.transform_mut(head).unwrap().scale = Vec3::new(1.0, 1.0, -1.0);
    });
    let gw = play(scene, 5);
    let s = gw.scene().borrow();
    let head = s.find_bone(rig, "head").unwrap();
    let drawn = s.world.local_to_world(head).transform_vector3(Vec3::Z);
    let want = target - bone_at(&s, rig, "head");
    let off = drawn.angle_between(want).to_degrees();
    assert!(off < 0.1, "aims {off}° off the target");
}
