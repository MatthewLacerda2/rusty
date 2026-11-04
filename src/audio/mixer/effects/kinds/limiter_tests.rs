//! The limiter against a sine (#1748): nothing past the ceiling, nothing touched
//! under it, and a NaN played as silence.

use kira::Frame;

use super::super::super::gain;
use super::super::signal::{run, sine, values};

#[test]
fn a_sine_twice_full_scale_never_passes_the_ceiling() {
    let mut s = sine(100.0, 2.0, 9600);
    run("limiter", &values("limiter", &[], &[]), &mut s);
    let ceiling = gain(-0.3);
    let peak = s
        .iter()
        .fold(0.0f32, |p, f| p.max(f.left.abs()).max(f.right.abs()));
    assert!(peak <= ceiling * (1.0 + 1e-6), "peak {peak} over {ceiling}");
    assert!(peak > ceiling * 0.99, "and reaches it: {peak}");
}

#[test]
fn under_the_ceiling_it_is_bit_transparent() {
    let quiet = sine(440.0, 0.9, 4800);
    let mut s = quiet.clone();
    run("limiter", &values("limiter", &[], &[]), &mut s);
    assert_eq!(s, quiet);
}

#[test]
fn pre_gain_pushes_into_it_and_a_nan_is_silence() {
    let mut s = sine(440.0, 0.5, 4800);
    s[100] = Frame::new(f32::NAN, 0.0);
    run(
        "limiter",
        &values("limiter", &[("pre_gain", 12.0), ("ceiling", -6.0)], &[]),
        &mut s,
    );
    assert_eq!(s[100], Frame::ZERO);
    let peak = s.iter().fold(0.0f32, |p, f| p.max(f.left.abs()));
    assert!(
        peak <= gain(-6.0) * (1.0 + 1e-6) && peak > gain(-6.5),
        "{peak}"
    );
}
