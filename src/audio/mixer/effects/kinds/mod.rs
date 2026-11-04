//! src/audio/mixer/effects/kinds/ — the effect kinds, one file each (#1748).
//!
//! Each file holds one kind's [`EffectKind`] row (its name, its parameters and
//! their ranges: Unity's names where Unity has the effect, Godot's where it does
//! not) and the DSP that runs it. [`KINDS`] is the one list of them, sorted by
//! name; a new kind is a file here and a row there.
//!
//! [`EffectKind`]: super::EffectKind

pub mod compressor;
pub mod delay;
pub mod distortion;
pub mod eq;
pub mod limiter;

use kira::Frame;

use super::EffectKind;

/// Every effect kind, sorted by name. A [`KindId`](super::KindId) is an index here.
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

/// A frame's louder channel, as a positive level.
pub fn peak(f: &Frame) -> f32 {
    f.left.abs().max(f.right.abs())
}

#[cfg(test)]
pub(crate) mod signal;
