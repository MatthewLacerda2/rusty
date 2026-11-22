//! Sine and triangle are naive, band-limited enough by their own spectra; saw and square take
//! polyBLEP.

use std::f32::consts::TAU;

use super::pulse;
use crate::patch::Wave;

/// `phase` in cycles (`0..1`); `dt` is the step per sample, sizing polyBLEP's window; only a
/// square reads `width`.
pub(super) fn sample(wave: Wave, phase: f32, dt: f32, width: f32) -> f32 {
    match wave {
        Wave::Sine => (TAU * phase).sin(),
        Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
        Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Wave::Square => pulse::sample(phase, dt, width),
    }
}

/// The polyBLEP residual around the step at phase 0, within one `dt` either side; zero elsewhere.
pub(super) fn poly_blep(phase: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if phase < dt {
        let t = phase / dt;
        2.0 * t - t * t - 1.0
    } else if phase > 1.0 - dt {
        let t = (phase - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}
