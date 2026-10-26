//! Bloom's ops: the switch, the two strengths and the lens dirt (#1442), its
//! texture and its strength, each strength `≥ 0`.

use super::{Effect, Knob};
use crate::components::{Bloom, PostProcessingComponent};
use crate::core::paths::stored;
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

/// Set the lens dirt texture as a [`stored`] path; `None` or a blank one clears
/// it. The renderer logs once and draws no dirt when it is missing or unreadable.
pub fn set_dirt_texture(vc: &mut PostProcessingComponent, path: Option<String>) {
    vc.bloom.dirt_texture = path.map(|p| stored(&p)).filter(|p| !p.is_empty());
}

/// Set how much of the bloom the lens dirt adds, clamped to `≥ 0`.
pub fn set_dirt_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.dirt_intensity = non_negative(value);
}

pub const EFFECT: Effect = Effect {
    name: "bloom",
    field: effect!(bloom, |c, v: Bloom| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
        set_threshold(c, v.threshold);
        set_dirt_texture(c, v.dirt_texture);
        set_dirt_intensity(c, v.dirt_intensity);
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
        Knob {
            field: "dirt_intensity",
            api: Some("BloomDirtIntensity"),
            get: |vc| vc.bloom.dirt_intensity,
            set: set_dirt_intensity,
        },
    ],
};
