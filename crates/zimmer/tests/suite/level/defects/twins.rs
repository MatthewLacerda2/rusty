//! Twin jumps: a jump matched by one in reach, to a twentieth, is the waveform's own edge and
//! not a click.

use super::{found, RATE};

/// Values this large keep every sample exact, and a twentieth of the 20 jump is exactly 1.
fn two_jumps(before: f32, after: f32) -> Vec<f32> {
    let (p, q) = (RATE as usize / 4, RATE as usize / 4 + 480);
    let mut signal = vec![0.0; RATE as usize / 2];
    let ramp = |from: f32, to: f32, at: usize, len: usize, signal: &mut [f32]| {
        for k in 0..len {
            signal[at + k] = from + (to - from) * k as f32 / len as f32;
        }
    };
    signal[p..p + 6].fill(20.0);
    ramp(20.0, before, p + 6, q - p - 12, &mut signal);
    signal[q - 6..q].fill(before);
    signal[q..q + 6].fill(after);
    ramp(after, 0.0, q + 6, 460, &mut signal);
    signal
}

#[test]
fn a_twin_matches_either_side_to_a_twentieth_of_the_jump() {
    let twins = found(1, &two_jumps(1.0, 21.0));
    assert_eq!(
        twins.clicks.count, 0,
        "a twentieth off either side: {twins:?}"
    );
    for (before, after) in [(1.5, 20.0), (0.0, 21.5)] {
        let defects = found(1, &two_jumps(before, after));
        assert!(
            defects.clicks.count >= 1,
            "{before} before, {after} after: {defects:?}"
        );
    }
}

/// The last period is the whole 35 ms reach, 1680 samples: that reset's only twin is behind it.
#[test]
fn a_twin_the_whole_reach_behind_is_a_twin() {
    let periods = [1_600, 1_600, 1_680, 1_680];
    let mut signal: Vec<f32> = periods
        .iter()
        .flat_map(|&period| (0..period).map(move |i| i as f32 / period as f32 * 0.8 - 0.4))
        .collect();
    signal.extend((0..800).map(|i| i as f32 / 1_680.0 * 0.8 - 0.4));
    signal.extend(std::iter::repeat_n(
        signal[signal.len() - 1],
        RATE as usize / 10,
    ));
    let defects = found(1, &signal);
    assert_eq!(defects.clicks.count, 0, "{defects:?}");
}

#[test]
fn a_twin_is_matched_three_before_and_two_after() {
    let mut signal = two_jumps(0.0, 20.0);
    let q = RATE as usize / 4 + 480;
    signal[q - 2..q].fill(5.0);
    let first = 0.25;
    let defects = found(1, &signal);
    assert!(
        defects
            .clicks
            .worst
            .iter()
            .all(|click| (click.at_seconds - first).abs() > 0.001),
        "{defects:?}"
    );
}

fn opening_reset(opening: usize, period: usize) -> Vec<f32> {
    let ramp = |len: usize| (0..len).map(move |i| i as f32 / len as f32 * 0.8 - 0.4);
    let mut signal: Vec<f32> = ramp(opening)
        .chain(ramp(period))
        .chain(ramp(period))
        .collect();
    signal.extend(ramp(period).take(800));
    signal.extend(std::iter::repeat_n(
        signal[signal.len() - 1],
        RATE as usize / 10,
    ));
    signal
}

/// A twin ahead counts once its two samples after are in, so the reach ahead is 1678, not 1680.
#[test]
fn a_twin_ahead_is_seen_two_samples_short_of_the_reach() {
    let twinned = found(1, &opening_reset(4_000, 1_678));
    assert_eq!(twinned.clicks.count, 0, "{twinned:?}");
    let alone = found(1, &opening_reset(4_000, 1_679));
    assert_eq!(alone.clicks.count, 1, "{alone:?}");
    assert_eq!(alone.clicks.worst[0].at_seconds, 4_000.0 / f64::from(RATE));
}

#[test]
fn a_twin_is_matched_the_same_before_the_window_fills() {
    let twinned = found(1, &opening_reset(1_000, 1_678));
    assert_eq!(twinned.clicks.count, 0, "{twinned:?}");
    let alone = found(1, &opening_reset(1_000, 1_679));
    assert_eq!(alone.clicks.count, 1, "{alone:?}");
    assert_eq!(alone.clicks.worst[0].at_seconds, 1_000.0 / f64::from(RATE));
}
