//! The same defect in several channels is one defect, placed and sized by the
//! worst of them; defects that do not overlap stay apart.

use super::{RATE, found, interleaved};

/// A second of stereo silence with an impulse of `left` at `at` in the left
/// channel and one of `right` `late` samples later in the right.
fn impulses(at: usize, left: f32, late: usize, right: f32) -> Vec<f32> {
    let mut l = vec![0.0; RATE as usize];
    let mut r = l.clone();
    l[at] = left;
    r[at + late] = right;
    interleaved(&l, &r)
}

/// The same click in both channels, the right a few samples late and exactly
/// as large: one click in every channel, placed where it was first heard. A
/// tie keeps the earlier moment, so the answer does not depend on which
/// channel is read last.
#[test]
fn a_click_in_both_channels_is_placed_at_the_first() {
    let at = RATE as usize / 2;
    let defects = found(2, &impulses(at, 0.3, 10, 0.3));
    assert_eq!(defects.clicks.count, 1, "{defects:?}");
    let click = &defects.clicks.worst[0];
    assert_eq!(click.channel, None, "{click:?}");
    assert_eq!(click.at_seconds, at as f64 / f64::from(RATE), "{click:?}");
}

/// Joined, the click is the larger of the two, where the larger one is.
#[test]
fn a_click_in_both_channels_is_the_larger_of_them() {
    let at = RATE as usize / 2;
    for (left, right, worst_at) in [(0.3, 0.1, at), (0.1, 0.3, at + 10)] {
        let defects = found(2, &impulses(at, left, 10, right));
        assert_eq!(defects.clicks.count, 1, "{defects:?}");
        let click = &defects.clicks.worst[0];
        let size_db = 20.0 * (0.3f64 / 1e-4).log10();
        assert!((click.size_db - size_db).abs() < 0.01, "{click:?}");
        assert_eq!(click.at_seconds, worst_at as f64 / f64::from(RATE));
    }
}

/// A click occupies 5 ms either side of it: clicks in two channels 10 ms
/// apart only touch, and are two; a sample nearer, they overlap and are one.
#[test]
fn clicks_in_two_channels_that_only_touch_are_two() {
    let at = RATE as usize / 2;
    for (late, count) in [(480, 2), (479, 1)] {
        let defects = found(2, &impulses(at, 0.3, late, 0.3));
        assert_eq!(defects.clicks.count, count, "{late} late: {defects:?}");
    }
}
