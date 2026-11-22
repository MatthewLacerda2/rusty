pub mod compressor;
pub mod delay;
pub mod distortion;
pub mod eq;
pub mod limiter;

use kira::Frame;

use super::EffectKind;

pub const KINDS: [&EffectKind; 5] = [
    &compressor::KIND,
    &delay::KIND,
    &distortion::KIND,
    &eq::KIND,
    &limiter::KIND,
];

/// A one-pole smoother's per-frame coefficient for a time constant of `ms`.
pub fn smoothing(ms: f32, dt: f64) -> f32 {
    if ms <= 0.0 {
        return 1.0;
    }
    (1.0 - (-dt / (f64::from(ms) / 1000.0)).exp()) as f32
}

pub fn peak(f: &Frame) -> f32 {
    f.left.abs().max(f.right.abs())
}

#[cfg(test)]
pub(crate) mod signal;
