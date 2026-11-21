//! One sample of a waveform at a phase, with the band-limiting step correction (polyBLEP).

use std::f32::consts::TAU;

use super::pulse;
use crate::patch::Wave;

pub(super) fn sample(wave: Wave, phase: f32, dt: f32, width: f32) -> f32 {
    match wave {
        Wave::Sine => (TAU * phase).sin(),
        Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
        Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Wave::Square => pulse::sample(phase, dt, width),
    }
}

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
