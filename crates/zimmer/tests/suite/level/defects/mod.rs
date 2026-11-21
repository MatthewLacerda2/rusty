//! Defects that live between samples: clicks, dropouts and stepped gain.
//!
//! Every signal here is built so the answer is known by construction — a jump
//! put at a named sample, a gap of a named length, a gain changed once every
//! 800 samples — and every clean signal is one a careless detector would flag:
//! a sawtooth is a jump every period, a low tone ripples, a smooth fade moves
//! the level all the time, and noise is nothing but jumps.

use std::f64::consts::TAU;

use scorsese_zimmer::level::{DefectFinder, Defects, Tally};

mod clicks;
mod dropouts;
mod found;
mod joined;
mod neighbours;
mod pulses;
mod reach;
mod stepped;
mod steps;
mod trains;

pub(super) const RATE: u32 = 48_000;

/// `seconds` of a sine at `hz` and `amplitude`, gained by `gain(t)` in dB.
pub(super) fn tone(hz: f64, amplitude: f64, seconds: f64, gain: impl Fn(f64) -> f64) -> Vec<f32> {
    let samples = (seconds * f64::from(RATE)) as usize;
    (0..samples)
        .map(|i| {
            let t = i as f64 / f64::from(RATE);
            (amplitude * (TAU * hz * t).sin() * 10f64.powf(gain(t) / 20.0)) as f32
        })
        .collect()
}

pub(super) fn steady(_: f64) -> f64 {
    0.0
}

pub(super) fn found(channels: usize, samples: &[f32]) -> Defects {
    let mut finder = DefectFinder::new(channels, RATE);
    finder.feed(samples);
    finder.finish()
}

pub(super) fn interleaved(left: &[f32], right: &[f32]) -> Vec<f32> {
    left.iter().zip(right).flat_map(|(l, r)| [*l, *r]).collect()
}

/// A deterministic white noise, so the assertion is the same on every run.
pub(super) fn noise(amplitude: f32, samples: usize) -> Vec<f32> {
    let mut state: u32 = 0x1234_5678;
    (0..samples)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32 * 2.0 - 1.0) * amplitude
        })
        .collect()
}

#[test]
fn clean_signals_have_no_defects() {
    for (name, signal) in [
        ("a 440 Hz tone", tone(440.0, 0.5, 2.0, steady)),
        ("a 60 Hz tone", tone(60.0, 0.5, 2.0, steady)),
        (
            "a smooth fade of 20 dB",
            tone(440.0, 0.5, 2.0, |t| -10.0 * t),
        ),
        ("a smooth bass fade", tone(80.0, 0.5, 2.0, |t| -10.0 * t)),
        (
            "a 110 Hz sawtooth",
            (0..2 * RATE as usize)
                .map(|i| ((i as f64 * 110.0 / f64::from(RATE)).fract() * 0.8 - 0.4) as f32)
                .collect(),
        ),
        (
            "a 32.7 Hz sawtooth, zimmer's lowest bass note",
            (0..2 * RATE as usize)
                .map(|i| ((i as f64 * 32.7 / f64::from(RATE)).fract() * 0.8 - 0.4) as f32)
                .collect(),
        ),
        ("white noise", noise(0.3, 2 * RATE as usize)),
        ("silence", vec![0.0; 2 * RATE as usize]),
    ] {
        let defects = found(1, &signal);
        assert!(defects.is_clean(), "{name}: {defects:?}");
    }
}

/// Clean is nothing of *any* kind: one of a kind alone is not clean, and nor
/// are two kinds whose counts would cancel or multiply away.
#[test]
fn a_defect_of_any_kind_is_not_clean() {
    let one = || Tally {
        count: 1,
        worst: Vec::new(),
    };
    assert!(Defects::default().is_clean());
    for defects in [
        Defects {
            clicks: one(),
            ..Defects::default()
        },
        Defects {
            dropouts: one(),
            ..Defects::default()
        },
        Defects {
            zipper: one(),
            ..Defects::default()
        },
        Defects {
            clicks: one(),
            dropouts: one(),
            ..Defects::default()
        },
        Defects {
            dropouts: one(),
            zipper: one(),
            ..Defects::default()
        },
    ] {
        assert!(!defects.is_clean(), "{defects:?}");
    }
}
