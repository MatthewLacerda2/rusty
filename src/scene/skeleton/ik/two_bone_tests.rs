//! The two-bone solver (#461): reach exactly, straighten toward the unreachable,
//! bend toward the hint, weight 0 is a no-op.

use glam::{Quat, Vec3};

use super::{solve_two_bone, BonePose};

/// An arm along +X from the origin: shoulder, elbow at 1, hand at 2, the elbow
/// slightly bent toward -Z so the limb has a plane.
fn arm() -> (BonePose, BonePose, Vec3) {
    let shoulder = BonePose {
        pos: Vec3::ZERO,
        rot: Quat::IDENTITY,
    };
    let elbow = BonePose {
        pos: Vec3::new(1.0, 0.0, -0.1),
        rot: Quat::IDENTITY,
    };
    (shoulder, elbow, Vec3::new(2.0, 0.0, 0.0))
}

/// Where the elbow and hand land once `root`/`mid` take the solved rotations
/// (bone vectors are carried by the change of each joint's world rotation).
fn pose(root: BonePose, mid: BonePose, tip: Vec3, solved: (Quat, Quat)) -> (Vec3, Vec3) {
    let elbow = root.pos + solved.0 * root.rot.inverse() * (mid.pos - root.pos);
    let hand = elbow + solved.1 * mid.rot.inverse() * (tip - mid.pos);
    (elbow, hand)
}

#[test]
fn a_reachable_target_is_reached_exactly() {
    let (s, e, h) = arm();
    let target = Vec3::new(0.6, 1.2, 0.3);
    let solved = solve_two_bone(s, e, h, target, None, 1.0).unwrap();
    let (elbow, hand) = pose(s, e, h, solved);
    assert!(hand.abs_diff_eq(target, 1e-4), "hand at {hand:?}");
    // Bone lengths are kept.
    assert!(((elbow - s.pos).length() - (e.pos - s.pos).length()).abs() < 1e-5);
    assert!(((hand - elbow).length() - (h - e.pos).length()).abs() < 1e-5);
}

#[test]
fn an_unreachable_target_gets_a_straight_arm_pointing_at_it() {
    let (s, e, h) = arm();
    let target = Vec3::new(0.0, 10.0, 0.0);
    let (elbow, hand) = pose(s, e, h, solve_two_bone(s, e, h, target, None, 1.0).unwrap());
    let reach = (e.pos - s.pos).length() + (h - e.pos).length();
    assert!(hand.abs_diff_eq(Vec3::Y * reach, 1e-3), "hand at {hand:?}");
    assert!(
        elbow.normalize().abs_diff_eq(Vec3::Y, 1e-3),
        "elbow at {elbow:?}"
    );
}

#[test]
fn the_elbow_bends_toward_the_hint() {
    let (s, e, h) = arm();
    let target = Vec3::new(1.2, 0.0, 0.0);
    for hint in [Vec3::new(0.5, 3.0, 0.0), Vec3::new(0.5, -3.0, 0.0)] {
        let solved = solve_two_bone(s, e, h, target, Some(hint), 1.0).unwrap();
        let (elbow, hand) = pose(s, e, h, solved);
        assert!(hand.abs_diff_eq(target, 1e-4));
        assert!(
            elbow.y.signum() == hint.y.signum() && elbow.z.abs() < 1e-4,
            "elbow {elbow:?} must bend toward {hint:?} in its plane"
        );
    }
}

#[test]
fn a_straight_limb_still_bends_toward_its_hint() {
    let (s, mut e, h) = arm();
    e.pos.z = 0.0; // animated perfectly straight: no plane of its own
    let hint = Vec3::new(1.0, 0.0, 2.0);
    let solved = solve_two_bone(s, e, h, Vec3::new(1.0, 0.0, 0.0), Some(hint), 1.0).unwrap();
    let (elbow, hand) = pose(s, e, h, solved);
    assert!(hand.abs_diff_eq(Vec3::X, 1e-4), "hand at {hand:?}");
    assert!(elbow.z > 0.5, "elbow {elbow:?} must bend toward +Z");
}

#[test]
fn weight_zero_is_a_no_op_and_half_weight_reaches_half_way() {
    let (s, e, h) = arm();
    let target = Vec3::new(1.0, 1.0, 0.0);
    assert_eq!(solve_two_bone(s, e, h, target, None, 0.0), None);
    let solved = solve_two_bone(s, e, h, target, None, 0.5).unwrap();
    let (_, hand) = pose(s, e, h, solved);
    assert!(
        hand.abs_diff_eq(h.lerp(target, 0.5), 1e-4),
        "hand at {hand:?}"
    );
}
