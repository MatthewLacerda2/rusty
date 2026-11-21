//! Sounds that pulse on their own, which a staircase detector must not take
//! for stepped gain: notes rich in harmonics, beats and a level that climbs
//! within every frame, each with a fade under it.

use super::{RATE, found, steady, tone};

/// A note rich in harmonics pulses once a cycle, at a period in the same
/// range a game's control rate is, and as it dies away the pulse can read as
/// a staircase. Stepped gain is not reported on a sound that pulses on its
/// own: it misses there rather than invents.
#[test]
fn a_low_note_dying_away_is_not_a_staircase() {
    for hz in [60.0, 120.0, 180.0, 250.0, 270.0, 339.0, 523.0] {
        for decay in [3.0, 10.0, 30.0, 60.0] {
            let signal = buzz(hz, 1.0, |t| -decay * t);
            let defects = found(1, &signal);
            assert_eq!(
                defects.zipper.count, 0,
                "{hz} Hz at -{decay} dB/s: {defects:?}"
            );
        }
    }
}

/// A dropout in a note dying away is not a staircase either: the note's pulse
/// is still there on both sides of the gap, and the gap itself must not
/// drown it out.
#[test]
fn a_note_with_a_dropout_dying_away_is_not_a_staircase() {
    for hz in [339.0, 440.0, 523.0, 659.0] {
        for decay in [10.0, 20.0, 30.0, 40.0] {
            for gap_ms in [4.0, 5.0, 6.0, 8.0] {
                let mut signal = buzz(hz, 1.0, |t| -decay * t);
                let at = RATE as usize / 2;
                let gap = (gap_ms * f64::from(RATE) / 1000.0) as usize;
                signal[at..at + gap].fill(0.0);
                let defects = found(1, &signal);
                assert_eq!(
                    defects.zipper.count, 0,
                    "{hz} Hz at -{decay} dB/s with a {gap_ms} ms gap: {defects:?}"
                );
            }
        }
    }
}

/// A note with a sharp pulse a cycle, as a voice or a reed has: the first
/// eight harmonics of `hz`, gained by `gain(t)` in dB.
fn buzz(hz: f64, seconds: f64, gain: impl Fn(f64) -> f64) -> Vec<f32> {
    let samples = (seconds * f64::from(RATE)) as usize;
    (0..samples)
        .map(|i| {
            let t = i as f64 / f64::from(RATE);
            let wave: f64 = (1..=8)
                .map(|n| (std::f64::consts::TAU * hz * f64::from(n) * t).cos() / 8.0)
                .sum();
            (0.6 * wave * 10f64.powf(gain(t) / 20.0)) as f32
        })
        .collect()
}

/// Two notes a few tens of hertz apart beat, and as they die away the beat
/// can read as a staircase at its own period: halved, each beat's halves are
/// level, but quartered they are not. A beat is not stepped gain, at any
/// decay.
#[test]
fn two_notes_beating_as_they_fade_are_not_a_staircase() {
    for (low, high) in [
        (220.0, 250.0),
        (330.0, 370.0),
        (440.0, 410.0),
        (440.0, 460.0),
        (440.0, 470.0),
        (440.0, 480.0),
        (440.0, 500.0),
        (523.0, 554.0),
    ] {
        for decay in [20.0, 30.0, 40.0, 60.0] {
            let gain = |t: f64| 10f64.powf(-decay * t / 20.0);
            let signal: Vec<f32> = tone(low, 0.3, 2.0, steady)
                .into_iter()
                .zip(tone(high, 0.3, 2.0, steady))
                .enumerate()
                .map(|(i, (a, b))| {
                    ((f64::from(a) + f64::from(b)) * gain(i as f64 / f64::from(RATE))) as f32
                })
                .collect();
            let defects = found(1, &signal);
            assert_eq!(
                defects.zipper.count, 0,
                "{low} and {high} Hz at -{decay} dB/s: {defects:?}"
            );
        }
    }
}

/// A level that drops at every frame and climbs back part of the way inside
/// it, in two quarter-frame steps, fades frame by frame but is not flat
/// within a frame. Halved at the frame it reads as a staircase; quartered,
/// or cut half a frame later, it does not, and it is not one.
#[test]
fn a_level_that_climbs_within_every_frame_is_not_a_staircase() {
    for (frame_ms, step_db) in [(12.0, 1.0), (12.0, 2.0), (16.6667, 0.5), (16.6667, 1.0)] {
        let frame = frame_ms / 1000.0;
        let signal = tone(440.0, 0.5, 1.5, |t| {
            let into = (t / frame).fract();
            let climb = if into < 0.25 {
                -step_db
            } else if into < 0.75 {
                0.0
            } else {
                step_db
            };
            -step_db * (t / frame).floor() + climb
        });
        let defects = found(1, &signal);
        assert_eq!(
            defects.zipper.count, 0,
            "{frame_ms} ms frames of {step_db} dB: {defects:?}"
        );
    }
}
