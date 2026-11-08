//! The reaction bookkeeping on its own: easing, overlapping reactions blending
//! with no jump,
//! and which bones are handed back when.

use glam::{Quat, Vec3};

use super::{Drive, RagdollRuntime};

const SOFT: Drive = Drive {
    strength: 100.0,
    damping: 20.0,
};
const HARD: Drive = Drive {
    strength: 900.0,
    damping: 60.0,
};

#[test]
fn a_reaction_eases_its_weight_to_zero_then_releases_its_bones() {
    let mut rt = RagdollRuntime::default();
    rt.start(4, vec![4, 5], &[4, 5], (0.4, SOFT));
    assert_eq!(rt.weight(5), 1.0);
    rt.advance(0.2);
    assert!(
        (rt.weight(5) - 0.5).abs() < 1e-6,
        "halfway: {}",
        rt.weight(5)
    );
    assert!(rt.release().is_empty(), "still running");
    rt.advance(0.2);
    assert_eq!(rt.weight(5), 0.0, "the last step is all animation");
    assert_eq!(rt.release(), vec![4, 5]);
    assert!(!rt.is_reacting(4));
    assert_eq!(rt.weight(4), 1.0, "free again: the explicit default");
}

#[test]
fn a_newer_inner_reaction_under_a_fading_outer_one_never_jumps() {
    // A chest hit (branch 4..5), then an arm hit (branch 5) 0.3 s later.
    let mut rt = RagdollRuntime::default();
    rt.start(4, vec![4, 5], &[4, 5], (0.4, SOFT));
    rt.advance(0.3);
    rt.start(5, vec![5], &[5], (0.4, HARD));
    assert_eq!(rt.weight(5), 1.0, "the arm's fresh reaction draws it");
    assert_eq!(rt.drive_override(5), Some(HARD));
    assert!(rt.weight(4) < 0.2, "the chest keeps fading");
    let mut last = rt.weight(5);
    let mut released = Vec::new();
    for _ in 0..30 {
        rt.advance(1.0 / 60.0);
        let w = rt.weight(5);
        if rt.is_reacting(5) {
            assert!((w - last).abs() < 0.07, "no frame jumps: {last} -> {w}");
        }
        last = w;
        released.extend(rt.release());
    }
    assert_eq!(
        released,
        vec![4, 5],
        "the chest first, the arm when it ends"
    );
}

#[test]
fn a_repeated_hit_restarts_and_a_script_switch_forgets() {
    let mut rt = RagdollRuntime::default();
    rt.start(4, vec![4], &[4], (0.4, SOFT));
    rt.advance(0.35);
    rt.start(4, vec![4], &[4], (0.4, SOFT));
    assert_eq!(rt.weight(4), 1.0, "restarted");
    rt.forget(&[4]);
    assert!(!rt.is_reacting(4) && rt.release().is_empty());
    rt.set_weight(4, 0.25);
    assert_eq!(rt.weight(4), 0.25);
}

#[test]
fn only_a_tracked_bone_takes_the_animators_pose() {
    let mut rt = RagdollRuntime::default();
    assert!(rt.is_idle());
    let pose = (Vec3::X, Quat::IDENTITY);
    rt.note_animated(7, pose);
    assert_eq!(rt.animated(7), None);
    rt.track(7, (Vec3::ZERO, Quat::IDENTITY));
    rt.track(7, (Vec3::Y, Quat::IDENTITY));
    assert_eq!(
        rt.animated(7).unwrap().0,
        Vec3::ZERO,
        "the first seed stays"
    );
    rt.note_animated(7, pose);
    assert_eq!(rt.animated(7), Some(pose));
    rt.retain(|b| b != 7);
    assert!(rt.is_idle());
}

#[test]
fn a_partial_drive_keeps_what_it_omits() {
    let own = Drive {
        strength: 900.0,
        damping: 60.0,
    };
    assert_eq!(
        own.merged(None, Some(80.0)),
        Drive {
            damping: 80.0,
            ..own
        }
    );
    assert_eq!(
        own.merged(Some(900.0), None),
        own,
        "same strength, same damping"
    );
    let stronger = own.merged(Some(1600.0), None);
    assert_eq!(
        stronger.damping, 80.0,
        "a new strength takes critical damping"
    );
    assert_eq!(own.merged(Some(-1.0), Some(f32::NAN)).strength, 0.0);
}
