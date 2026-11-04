//! The delay against an impulse (#1748): the echo's time, its level, each repeat's
//! decay and the feedback low-pass.

use kira::Frame;

use super::super::signal::{run, values};

/// An impulse, then silence, `frames` long.
fn impulse(frames: usize) -> Vec<Frame> {
    let mut s = vec![Frame::ZERO; frames];
    s[0] = Frame::new(1.0, 0.5);
    s
}

#[test]
fn an_impulse_echoes_at_the_delay_and_each_repeat_decays() {
    let v = values(
        "delay",
        &[("delay", 10.0), ("decay", 0.5), ("wet", 0.8), ("dry", 1.0)],
        &[],
    );
    let mut s = impulse(2000);
    run("delay", &v, &mut s);
    assert_eq!(s[0], Frame::new(1.0, 0.5), "dry");
    assert!(
        (s[480].left - 0.8).abs() < 1e-6,
        "first echo at 10 ms: {}",
        s[480].left
    );
    assert!((s[480].right - 0.4).abs() < 1e-6);
    assert!(
        (s[960].left - 0.4).abs() < 1e-6,
        "second at half: {}",
        s[960].left
    );
    let between = s[1..480].iter().chain(&s[481..960]);
    assert!(between.into_iter().all(|f| f.left == 0.0));
}

#[test]
fn the_feedback_low_pass_darkens_repeats_and_dry_zero_leaves_only_echo() {
    let v = values(
        "delay",
        &[
            ("delay", 10.0),
            ("decay", 1.0),
            ("dry", 0.0),
            ("low_pass", 500.0),
        ],
        &[],
    );
    let mut s = impulse(2000);
    run("delay", &v, &mut s);
    assert_eq!(s[0], Frame::ZERO, "no dry");
    assert!(
        (s[480].left - 1.0).abs() < 1e-6,
        "the first echo is the line"
    );
    let second = s[950..1000].iter().fold(0.0f32, |p, f| p.max(f.left));
    assert!(
        second < 0.1,
        "a 500 Hz feedback smears the impulse: {second}"
    );
    assert!(second > 0.0);
}
