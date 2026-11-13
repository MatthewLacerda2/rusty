//! The distortion's three modes against a sine (#1748).

use super::super::signal::{run, sine, values};
use super::shape;

#[test]
fn at_no_drive_every_mode_is_a_straight_wire() {
    let clean = sine(440.0, 0.7, 2400);
    for mode in ["soft", "hard", "bitcrush"] {
        let mut s = clean.clone();
        run(
            "distortion",
            &values("distortion", &[], &[("mode", mode)]),
            &mut s,
        );
        assert_eq!(s, clean, "{mode}");
    }
}

#[test]
fn hard_clip_flattens_at_full_scale_and_soft_rounds_under_it() {
    let mut hard = sine(100.0, 0.5, 4800);
    run(
        "distortion",
        &values("distortion", &[("drive", 0.5)], &[("mode", "hard")]),
        &mut hard,
    );
    let at_top = hard.iter().filter(|f| f.left.abs() == 1.0).count();
    assert!(
        at_top > 1000,
        "a 13x push clips most of the cycle: {at_top}"
    );
    let mut soft = sine(100.0, 0.5, 4800);
    run(
        "distortion",
        &values("distortion", &[("drive", 0.5)], &[("mode", "soft")]),
        &mut soft,
    );
    assert!(soft.iter().all(|f| f.left.abs() < 1.0));
    assert!(
        soft.iter().any(|f| f.left.abs() > 0.99),
        "pushed up toward full scale"
    );
    assert_eq!(shape(0, 1.0, 1.0), 1.0, "full scale stays full scale");
}

#[test]
fn bitcrush_at_full_drive_leaves_three_levels_and_post_gain_scales() {
    let mut s = sine(100.0, 0.9, 4800);
    run(
        "distortion",
        &values(
            "distortion",
            &[("drive", 1.0), ("post_gain", -6.0)],
            &[("mode", "bitcrush")],
        ),
        &mut s,
    );
    let mut levels: Vec<i32> = s.iter().map(|f| (f.left * 1000.0).round() as i32).collect();
    levels.sort_unstable();
    levels.dedup();
    assert_eq!(levels, [-501, 0, 501], "one bit, at −6 dB");
}
