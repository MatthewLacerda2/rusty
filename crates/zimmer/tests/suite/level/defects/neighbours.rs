//! What a jump's neighbours decide: a jump that has company in reach is not a
//! click, and jumps within one click's few milliseconds are one click.

use super::{RATE, found};

const REACH: usize = 1_680;

/// Silence stepping up by 0.2 at a quarter of a second and by 0.04 more
/// `after` samples later.
fn step_then_small_step(after: usize) -> Vec<f32> {
    let at = RATE as usize / 4;
    let mut signal = vec![0.0; RATE as usize / 2];
    signal[at..].fill(0.2);
    signal[at + after..].fill(0.24);
    signal
}

/// A click's size is read against the largest jump in reach, out to its
/// very edge, 35 ms on: a fifth of it there makes it 14 dB; a sample further
/// it has nothing to stand above but silence.
#[test]
fn a_click_is_sized_against_jumps_to_the_edge_of_reach() {
    for (after, size_db) in [
        (REACH, 20.0 * 5f64.log10()),
        (REACH + 1, 20.0 * 2e3f64.log10()),
    ] {
        let defects = found(1, &step_then_small_step(after));
        let click = defects
            .clicks
            .worst
            .iter()
            .find(|click| click.at_seconds == 0.25)
            .unwrap_or_else(|| panic!("{after}: no click at 0.25 s: {defects:?}"));
        assert!((click.size_db - size_db).abs() < 0.01, "{after}: {click:?}");
    }
}

/// A jump of 0.4 with two others half its size and a quarter, three samples
/// apart, 10 ms on: neither of those stands out from the other, so they are
/// company the first jump is not four times, and nothing is a click.
#[test]
fn a_cluster_of_jumps_is_company() {
    let at = RATE as usize / 4;
    let mut signal = vec![0.0; RATE as usize / 2];
    signal[at..].fill(0.4);
    signal[at + 480..].fill(0.6);
    signal[at + 483..].fill(0.7);
    let defects = found(1, &signal);
    assert_eq!(defects.clicks.count, 0, "{defects:?}");
}

/// Two steps 2 ms apart are one click, placed and sized by the larger, and by
/// the first of two that are equal.
#[test]
fn two_jumps_in_one_click_are_the_larger() {
    let at = RATE as usize / 4;
    for (first, second, worst) in [(0.1, 0.3, at + 96), (0.3, 0.1, at), (0.2, 0.2, at)] {
        let mut signal = vec![0.0; RATE as usize / 2];
        signal[at..].fill(first);
        signal[at + 96..].fill(first + second);
        let defects = found(1, &signal);
        assert_eq!(defects.clicks.count, 1, "{first}, {second}: {defects:?}");
        let click = &defects.clicks.worst[0];
        assert_eq!(
            click.at_seconds,
            worst as f64 / f64::from(RATE),
            "{click:?}"
        );
        let size_db = 20.0 * (f64::from(first.max(second)) / 1e-4).log10();
        assert!((click.size_db - size_db).abs() < 0.01, "{click:?}");
    }
}

/// A sawtooth that ends a reach and two samples after its last reset: the
/// signal's end changes nothing about a jump already judged, and the reset,
/// with its twin a whole reach behind, is still no click.
#[test]
fn the_end_of_the_signal_does_not_rejudge_a_jump() {
    let periods = [1_600, 1_600, 1_680, 1_680];
    let mut signal: Vec<f32> = periods
        .iter()
        .flat_map(|&period| (0..period).map(move |i| i as f32 / period as f32 * 0.8 - 0.4))
        .collect();
    signal.extend((0..REACH + 2).map(|i| i as f32 / 1_680.0 * 0.8 - 0.4));
    let defects = found(1, &signal);
    assert_eq!(defects.clicks.count, 0, "{defects:?}");
}

/// The two samples after a jump are its own: a step of 0.3 that rises 0.1
/// more two samples on and 0.05 more three after that is one click, sized
/// against the 0.05, the first jump that is not its own.
#[test]
fn the_two_samples_after_a_jump_are_its_own() {
    let at = RATE as usize / 4;
    let mut signal = vec![0.0; RATE as usize / 2];
    signal[at..].fill(0.3);
    signal[at + 2..].fill(0.4);
    signal[at + 5..].fill(0.45);
    let defects = found(1, &signal);
    assert_eq!(defects.clicks.count, 1, "{defects:?}");
    let click = &defects.clicks.worst[0];
    assert_eq!(click.at_seconds, 0.25, "{click:?}");
    assert!(
        (click.size_db - 20.0 * 6f64.log10()).abs() < 0.01,
        "{click:?}"
    );
}
