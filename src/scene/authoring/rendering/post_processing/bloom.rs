//! Bloom's ops: the switch and the two strengths, each `≥ 0`.

use super::{Effect, Knob};
use crate::components::{Bloom, PostProcessingComponent};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::non_negative;

/// Set whether bloom is active.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.bloom.active = active;
}

/// Set the bloom intensity, clamped to `≥ 0`.
pub fn set_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.intensity = non_negative(value);
}

/// Set the bloom threshold, clamped to `≥ 0`.
pub fn set_threshold(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.threshold = non_negative(value);
}

pub const EFFECT: Effect = Effect {
    name: "bloom",
    field: effect!(bloom, |c, v: Bloom| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
        set_threshold(c, v.threshold);
    }),
    knobs: &[
        Knob {
            field: "intensity",
            api: Some("BloomIntensity"),
            get: |vc| vc.bloom.intensity,
            set: set_intensity,
        },
        Knob {
            field: "threshold",
            api: Some("BloomThreshold"),
            get: |vc| vc.bloom.threshold,
            set: set_threshold,
        },
    ],
};
