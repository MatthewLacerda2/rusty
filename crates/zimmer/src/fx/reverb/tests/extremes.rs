//! The room at the ends of its knobs: dry, half wet, at no rate, and fed silence or nonsense.

use super::*;

#[test]
fn a_dry_mix_leaves_the_signal_alone() {
    assert_eq!(rung(4096, 0.7, 0.5, 0.0), impulse(4096));
}

#[test]
fn the_tail_is_blended_into_the_dry_signal_and_not_out_of_it() {
    let dry = impulse(8192);
    let wet = rung(8192, 0.8, 0.5, 1.0);
    let half = rung(8192, 0.8, 0.5, 0.5);
    let mut compared = 0;
    for ((h, d), w) in half.l.iter().zip(&dry.l).zip(&wet.l) {
        assert!((h - (d + w) * 0.5).abs() < 1e-6, "half of the way across");
        if w.abs() > 1e-4 {
            compared += 1;
        }
    }
    assert!(compared > 100, "only {compared} samples carried any tail");
}

#[test]
fn a_rate_of_nothing_falls_back_to_the_rate_the_tunings_were_chosen_at() {
    let rung_at = |rate: f32| {
        let mut buf = impulse(8192);
        apply(&mut buf, 0.8, 0.5, 0.0, 1.0, rate);
        buf
    };
    assert_eq!(rung_at(0.0), rung_at(TUNED_RATE));
    assert!(energy(&rung_at(0.0).l) > 0.5, "and it is still a room");
    assert_ne!(rung_at(TUNED_RATE * 2.0), rung_at(TUNED_RATE));
}

#[test]
fn silence_in_is_silence_out_and_stays_finite() {
    let mut buf = Stereo::silence(4096);
    apply(&mut buf, 0.9, 0.2, 0.0, 1.0, 44_100.0);
    assert!(buf.l.iter().chain(&buf.r).all(|s| *s == 0.0));
    let mut buf = impulse(4096);
    apply(&mut buf, 2.0, -1.0, 0.0, 3.0, 0.0);
    assert!(
        buf.l.iter().chain(&buf.r).all(|s| s.is_finite()),
        "clamped, not exploded"
    );
}
