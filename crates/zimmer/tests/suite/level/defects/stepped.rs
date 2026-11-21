//! Stepped gain: a level moving in flat steps instead of smoothly.

use super::{RATE, found, tone};

/// The defect the measurement was asked for: a gain set once per game frame
/// and held for the frame, so a fade arrives as a staircase.
#[test]
fn a_fade_stepped_at_a_control_rate_is_zipper_with_its_period() {
    let period = 800.0 / f64::from(RATE); // 16.67 ms, a 60 Hz control rate
    let signal = tone(440.0, 0.5, 2.0, |t| {
        if !(0.5..1.5).contains(&t) {
            return if t < 0.5 { 0.0 } else { -30.0 };
        }
        -0.5 - ((t - 0.5) / period).floor() * 0.5
    });
    let defects = found(1, &signal);
    assert!(defects.zipper.count >= 1, "{defects:?}");
    let run = &defects.zipper.worst[0];
    let found_period = run.period_seconds.expect("a zipper has a period");
    assert!((found_period - period).abs() < 0.0005, "{run:?}");
    assert!((run.size_db - 0.5).abs() < 0.2, "{run:?}");
    assert!(run.from_seconds < 0.6 && run.to_seconds > 1.4, "{run:?}");
    assert_eq!(defects.clicks.count, 0, "{defects:?}");
}

/// A fade over a quarter of a second — fifteen steps — is found on its own,
/// in a file where most of the sound is steady. Its last few steps may go
/// unclaimed: a window that reaches past the end of a staircase is mostly
/// flat, and only windows that are mostly staircase vouch for one.
#[test]
fn a_short_staircase_is_found_where_it_is() {
    let period = 800.0 / f64::from(RATE);
    let signal = tone(330.0, 0.5, 2.0, |t| {
        -2.0 * ((t - 1.0).max(0.0) / period).floor().min(15.0)
    });
    let defects = found(1, &signal);
    assert_eq!(defects.zipper.count, 1, "{defects:?}");
    let run = &defects.zipper.worst[0];
    assert!((run.size_db - 2.0).abs() < 0.3, "{run:?}");
    assert!((run.from_seconds - 1.0).abs() < 0.03, "{run:?}");
    assert!((1.15..1.27).contains(&run.to_seconds), "{run:?}");
}

/// A fade-out at the very end of a file is judged like one anywhere else,
/// whatever the file's length.
#[test]
fn a_staircase_at_the_end_of_a_file_is_found() {
    let period = 800.0 / f64::from(RATE);
    for seconds in [1.0, 2.0, 2.5] {
        let from = seconds - 0.35;
        let signal = tone(440.0, 0.5, seconds, |t| {
            -((t - from).max(0.0) / period).floor().min(20.0)
        });
        let defects = found(1, &signal);
        assert_eq!(defects.zipper.count, 1, "a {seconds} s file: {defects:?}");
    }
}

/// Noise is nothing but random change, and a staircase under it is still
/// found: the steps all go one way, and noise goes every way.
#[test]
fn noise_faded_in_steps_is_a_staircase() {
    let period = 800.0 / f64::from(RATE);
    let signal: Vec<f32> = super::noise(0.3, 3 * RATE as usize / 2)
        .into_iter()
        .enumerate()
        .map(|(i, x)| x * 10f32.powf(-(i as f32 / 800.0).floor() / 20.0))
        .collect();
    let defects = found(1, &signal);
    assert!(defects.zipper.count >= 1, "{defects:?}");
    let run = &defects.zipper.worst[0];
    assert!(
        (run.period_seconds.expect("a period") - period).abs() < 0.0005,
        "{run:?}"
    );
    assert!((run.size_db - 1.0).abs() < 0.3, "{run:?}");
}

/// A fade in large steps, one game frame apart, is a staircase as
/// plainly as one in small steps: its steps stand far clear of the noise, and
/// a segment's halves are as level as ever.
#[test]
fn a_fade_in_large_steps_is_a_staircase() {
    let period = 800.0 / f64::from(RATE);
    for hz in [220.0, 440.0] {
        let signal = tone(hz, 0.5, 2.0, |t| {
            -3.0 * ((t - 0.5).max(0.0) / period).floor().min(12.0)
        });
        let defects = found(1, &signal);
        assert_eq!(defects.zipper.count, 1, "{hz} Hz: {defects:?}");
        let run = &defects.zipper.worst[0];
        assert!((run.size_db - 3.0).abs() < 0.5, "{hz} Hz: {run:?}");
    }
}
