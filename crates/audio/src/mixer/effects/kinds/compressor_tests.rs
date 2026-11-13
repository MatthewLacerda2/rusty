//! The compressor against a sine (#1748): its static curve, its makeup, and a
//! sidechain key.

use std::sync::Arc;

use super::super::super::{DspContext, KeyLevel};
use super::super::signal::{peak_db, run, run_with, sine, values, RATE};

#[test]
fn a_sine_over_the_threshold_comes_out_on_the_ratio_curve() {
    // −6 dBFS through −20 dB at 4:1: 14 dB over leaves 3.5 dB over.
    let v = values(
        "compressor",
        &[
            ("threshold", -20.0),
            ("ratio", 4.0),
            ("attack", 1.0),
            ("release", 200.0),
        ],
        &[],
    );
    let mut s = sine(1000.0, 0.5, RATE as usize);
    run("compressor", &v, &mut s);
    let level = peak_db(&s[s.len() - 4800..]);
    assert!((level - -16.5).abs() < 0.5, "settled at {level} dB");
}

#[test]
fn under_the_threshold_it_is_bit_transparent_and_makeup_lifts_it() {
    let quiet = sine(440.0, 0.05, 4800);
    let mut s = quiet.clone();
    run(
        "compressor",
        &values("compressor", &[("threshold", -20.0)], &[]),
        &mut s,
    );
    assert_eq!(s, quiet, "−26 dBFS never crosses −20");
    let mut lifted = quiet.clone();
    run(
        "compressor",
        &values("compressor", &[("makeup", 6.0)], &[]),
        &mut lifted,
    );
    let gain = peak_db(&lifted) - peak_db(&quiet);
    assert!((gain - 6.0).abs() < 0.01, "{gain} dB");
}

#[test]
fn a_sidechain_key_compresses_by_the_other_groups_level() {
    let v = values(
        "compressor",
        &[("threshold", -30.0), ("ratio", 10.0), ("attack", 1.0)],
        &[],
    );
    let music = sine(200.0, 0.1, RATE as usize / 2);
    let keyed = |level: f32| {
        let key = Arc::new(KeyLevel::default());
        key.set(level);
        let mut s = music.clone();
        let ctx = DspContext {
            sample_rate: RATE,
            key: Some(key),
        };
        run_with("compressor", &v, &mut s, ctx);
        peak_db(&s[s.len() - 4800..])
    };
    let (silent, loud) = (keyed(0.0), keyed(1.0));
    assert!(
        (silent - -20.0).abs() < 0.1,
        "a silent key leaves it at {silent}"
    );
    // The key at 0 dB is 30 over; 10:1 takes 27 dB off the music.
    assert!((loud - -47.0).abs() < 0.5, "a loud key ducks it to {loud}");
}

#[test]
fn an_infinite_sample_does_not_poison_it() {
    let v = values("compressor", &[("threshold", -20.0)], &[]);
    let mut s = sine(200.0, 0.1, 9600);
    s[10] = kira::Frame::new(f32::INFINITY, 0.0);
    run("compressor", &v, &mut s);
    let after = peak_db(&s[4800..]);
    assert!(
        (after - -20.0).abs() < 0.1,
        "under the threshold again: {after}"
    );
}
