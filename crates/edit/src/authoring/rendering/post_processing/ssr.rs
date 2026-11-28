use super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use rusty_components::{PostProcessingComponent, Ssr, SsrAlgorithm, SsrTracing};
use rusty_core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use rusty_scene::rules::sanitize::clamped;

pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssr.active = active;
}

pub fn set_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.ssr.quality = quality;
}

pub fn set_steps(vc: &mut PostProcessingComponent, steps: u32) {
    vc.ssr.steps = steps.clamp(1, Ssr::MAX_STEPS);
}

pub fn set_step_length(vc: &mut PostProcessingComponent, length: f32) {
    vc.ssr.step_length = clamped(length, Ssr::MIN_STEP_LENGTH, Ssr::MAX_STEP_LENGTH);
}

pub fn set_min_smoothness(vc: &mut PostProcessingComponent, smoothness: f32) {
    vc.ssr.min_smoothness = clamped(smoothness, 0.0, 1.0);
}

pub fn set_algorithm(vc: &mut PostProcessingComponent, algorithm: SsrAlgorithm) {
    vc.ssr.algorithm = algorithm;
}

pub fn set_tracing(vc: &mut PostProcessingComponent, tracing: SsrTracing) {
    vc.ssr.tracing = tracing;
}

/// Held at or above `min_smoothness` when the trace reads it (`Ssr::ray_threshold`), not here:
/// lowering `min_smoothness` later keeps the value asked for.
pub fn set_ray_min_smoothness(vc: &mut PostProcessingComponent, smoothness: f32) {
    vc.ssr.ray_min_smoothness = clamped(smoothness, 0.0, 1.0);
}

pub fn set_ray_length(vc: &mut PostProcessingComponent, length: f32) {
    vc.ssr.ray_length = clamped(length, Ssr::MIN_RAY_LENGTH, Ssr::MAX_RAY_LENGTH);
}

pub fn set_trace_resolution_divisor(vc: &mut PostProcessingComponent, divisor: u32) {
    vc.ssr.trace_resolution_divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR);
}

pub const EFFECT: Effect = Effect {
    name: "ssr",
    field: effect!(ssr, |c, v: Ssr| {
        set_active(c, v.active);
        set_quality(c, v.quality);
        set_steps(c, v.steps);
        set_step_length(c, v.step_length);
        set_min_smoothness(c, v.min_smoothness);
        set_algorithm(c, v.algorithm);
        set_tracing(c, v.tracing);
        set_ray_min_smoothness(c, v.ray_min_smoothness);
        set_ray_length(c, v.ray_length);
        set_trace_resolution_divisor(c, v.trace_resolution_divisor);
    }),
    knobs: &[
        Knob {
            field: "step_length",
            api: Some("SsrStepLength"),
            get: |vc| vc.ssr.step_length,
            set: set_step_length,
        },
        Knob {
            field: "min_smoothness",
            api: Some("SsrMinSmoothness"),
            get: |vc| vc.ssr.min_smoothness,
            set: set_min_smoothness,
        },
        Knob {
            field: "ray_min_smoothness",
            api: Some("SsrRayMinSmoothness"),
            get: |vc| vc.ssr.ray_min_smoothness,
            set: set_ray_min_smoothness,
        },
        Knob {
            field: "ray_length",
            api: Some("SsrRayLength"),
            get: |vc| vc.ssr.ray_length,
            set: set_ray_length,
        },
    ],
};
