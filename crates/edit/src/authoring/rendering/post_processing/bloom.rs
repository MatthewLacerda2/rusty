use super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use rusty_components::{Bloom, PostProcessingComponent};
use rusty_core::paths::stored;
use rusty_core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use rusty_scene::rules::sanitize::{non_negative, unit};

pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.bloom.active = active;
}

pub fn set_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.intensity = non_negative(value);
}

pub fn set_threshold(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.threshold = non_negative(value);
}

pub fn set_scatter(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.scatter = unit(value);
}

pub fn set_dirt_texture(vc: &mut PostProcessingComponent, path: Option<String>) {
    vc.bloom.dirt_texture = path.map(|p| stored(&p)).filter(|p| !p.is_empty());
}

pub fn set_dirt_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.dirt_intensity = non_negative(value);
}

pub fn set_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.bloom.quality = quality;
}

pub fn set_resolution_divisor(vc: &mut PostProcessingComponent, divisor: u32) {
    vc.bloom.resolution_divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR);
}

pub const EFFECT: Effect = Effect {
    name: "bloom",
    field: effect!(bloom, |c, v: Bloom| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
        set_threshold(c, v.threshold);
        set_scatter(c, v.scatter);
        set_dirt_texture(c, v.dirt_texture);
        set_dirt_intensity(c, v.dirt_intensity);
        set_quality(c, v.quality);
        set_resolution_divisor(c, v.resolution_divisor);
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
            field: "scatter",
            api: Some("BloomScatter"),
            get: |vc| vc.bloom.scatter,
            set: set_scatter,
        },
        Knob {
            field: "dirt_intensity",
            api: Some("BloomDirtIntensity"),
            get: |vc| vc.bloom.dirt_intensity,
            set: set_dirt_intensity,
        },
    ],
};
