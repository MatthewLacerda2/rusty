//! What a jump's neighbours decide: a jump that has company in reach is not a click, and jumps
//! within one click's few milliseconds are one click.

use super::{found, RATE};

const REACH: usize = 1_680;

fn step_then_small_step(after: usize) -> Vec<f32> {
    let at = RATE as usize / 4;
    let mut signal = vec![0.0; RATE as usize / 2];
    signal[at..].fill(0.2);
    signal[at + after..].fill(0.24);
    signal
}

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
