//! Stepped gain is looked for in stretches of half a second, every quarter of
//! a second, plus one ending at the end: a staircase anywhere in a file is in
//! one of them, the first quarter-second and the last included.

use super::{RATE, found, tone};

/// A 330 Hz tone at 0.5 of `seconds`, stepping down 2 dB every 800 samples
/// (a 60 Hz control rate) `steps` times from `from` seconds on.
fn stepped(seconds: f64, from: f64, steps: f64) -> Vec<f32> {
    let period = 800.0 / f64::from(RATE);
    tone(330.0, 0.5, seconds, |t| {
        -2.0 * ((t - from).max(0.0) / period).floor().min(steps)
    })
}

#[test]
fn a_staircase_in_the_first_quarter_second_is_found() {
    let defects = found(1, &stepped(1.0, 0.02, 14.0));
    assert_eq!(defects.zipper.count, 1, "{defects:?}");
    assert!(defects.zipper.worst[0].from_seconds < 0.05, "{defects:?}");
}

/// 2559 blocks of half a millisecond: the stretches every 512 blocks stop
/// 511 short of the end, the most they can, and the staircase is in those.
#[test]
fn a_staircase_past_the_last_whole_stretch_is_found() {
    let seconds = 2_559.0 * 0.0005;
    let defects = found(1, &stepped(seconds, seconds - 0.25, 13.0));
    assert_eq!(defects.zipper.count, 1, "{defects:?}");
}

/// A mean square under −70 dB is silence and has no level: a staircase from
/// −53 dB down to −68 is judged, and one from −63 down to −78 is not.
#[test]
fn a_staircase_in_near_silence_is_not_judged() {
    let period = 800.0 / f64::from(RATE);
    let staircase = |amplitude: f64| {
        tone(330.0, amplitude, 2.0, |t| {
            -((t - 1.0).max(0.0) / period).floor().min(15.0)
        })
    };
    let heard = found(1, &staircase(3e-3));
    assert_eq!(heard.zipper.count, 1, "{heard:?}");
    let silent = found(1, &staircase(1e-3));
    assert_eq!(silent.zipper.count, 0, "{silent:?}");
}
