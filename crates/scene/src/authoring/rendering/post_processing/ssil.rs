//! Screen-space indirect lighting's ops (#1750): the switch, the intensity and the
//! radius (knobs), the quality level and the `Custom` sample count and resolution
//! divisor (counts: no tween).

use super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use crate::authoring::sanitize::clamped;
use rusty_components::{PostProcessingComponent, Ssil};
use rusty_core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};

/// Set whether SSIL is active.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssil.active = active;
}

/// Set how strongly the bounce lights a surface, clamped to `0..=4`.
pub fn set_intensity(vc: &mut PostProcessingComponent, intensity: f32) {
    vc.ssil.intensity = clamped(intensity, 0.0, Ssil::MAX_INTENSITY);
}

/// Set the gather's reach in world units, clamped to `0.1..=10`.
pub fn set_radius(vc: &mut PostProcessingComponent, radius: f32) {
    vc.ssil.radius = clamped(radius, Ssil::MIN_RADIUS, Ssil::MAX_RADIUS);
}

/// Set which samples and resolution SSIL runs: a preset level, or `Custom`.
pub fn set_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.ssil.quality = quality;
}

/// Set the `Custom` sample count, clamped to `1..=32`.
pub fn set_samples(vc: &mut PostProcessingComponent, samples: u32) {
    vc.ssil.samples = samples.clamp(1, Ssil::MAX_SAMPLES);
}

/// Set the `Custom` resolution divisor, clamped to `1..=4`.
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
