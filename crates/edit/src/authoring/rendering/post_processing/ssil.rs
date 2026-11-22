use super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use rusty_components::{PostProcessingComponent, Ssil};
use rusty_core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use rusty_scene::rules::sanitize::clamped;

pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssil.active = active;
}

pub fn set_intensity(vc: &mut PostProcessingComponent, intensity: f32) {
    vc.ssil.intensity = clamped(intensity, 0.0, Ssil::MAX_INTENSITY);
}

pub fn set_radius(vc: &mut PostProcessingComponent, radius: f32) {
    vc.ssil.radius = clamped(radius, Ssil::MIN_RADIUS, Ssil::MAX_RADIUS);
}

pub fn set_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.ssil.quality = quality;
}

pub fn set_samples(vc: &mut PostProcessingComponent, samples: u32) {
    vc.ssil.samples = samples.clamp(1, Ssil::MAX_SAMPLES);
}

pub fn set_resolution_divisor(vc: &mut PostProcessingComponent, divisor: u32) {
    vc.ssil.resolution_divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR);
}

pub const EFFECT: Effect = Effect {
    name: "ssil",
    field: effect!(ssil, |c, v: Ssil| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
        set_radius(c, v.radius);
        set_quality(c, v.quality);
        set_samples(c, v.samples);
        set_resolution_divisor(c, v.resolution_divisor);
    }),
    knobs: &[
        Knob {
            field: "intensity",
            api: Some("SsilIntensity"),
            get: |vc| vc.ssil.intensity,
            set: set_intensity,
        },
        Knob {
            field: "radius",
            api: Some("SsilRadius"),
            get: |vc| vc.ssil.radius,
            set: set_radius,
        },
    ],
};
