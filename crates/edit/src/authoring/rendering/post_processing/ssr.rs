//! Screen-space reflections' ops: the switch, the quality level (#1522) and the
//! `Custom` march, its step count and its step length (a knob: a count is no
//! tween), the *Minimum Smoothness* (a knob, #1736) and the *Object Thickness* (a
//! knob, #2228).

use super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use rusty_components::{PostProcessingComponent, Ssr};
use rusty_core::settings::quality::EffectQuality;
use rusty_scene::rules::sanitize::clamped;

/// Set whether SSR is active.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssr.active = active;
}

/// Set which march SSR runs: a level of the active quality preset, or `Custom`.
pub fn set_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.ssr.quality = quality;
}

/// Set the `Custom` march's step count, clamped to `1..=256`.
pub fn set_steps(vc: &mut PostProcessingComponent, steps: u32) {
    vc.ssr.steps = steps.clamp(1, Ssr::MAX_STEPS);
}

/// Set the `Custom` march's step length in world units, clamped to `0.05..=2`.
pub fn set_step_length(vc: &mut PostProcessingComponent, length: f32) {
    vc.ssr.step_length = clamped(length, Ssr::MIN_STEP_LENGTH, Ssr::MAX_STEP_LENGTH);
}

/// Set the *Minimum Smoothness* below which a surface gets no SSR, clamped to `0..=1`.
pub fn set_min_smoothness(vc: &mut PostProcessingComponent, smoothness: f32) {
    vc.ssr.min_smoothness = clamped(smoothness, 0.0, 1.0);
}

/// Set the *Object Thickness*, a fraction of the depth, clamped to `0..=1`.
pub fn set_thickness(vc: &mut PostProcessingComponent, thickness: f32) {
    vc.ssr.thickness = clamped(thickness, 0.0, Ssr::MAX_THICKNESS);
}

pub const EFFECT: Effect = Effect {
    name: "ssr",
    field: effect!(ssr, |c, v: Ssr| {
        set_active(c, v.active);
        set_quality(c, v.quality);
        set_steps(c, v.steps);
        set_step_length(c, v.step_length);
        set_min_smoothness(c, v.min_smoothness);
        set_thickness(c, v.thickness);
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
            field: "thickness",
            api: Some("SsrThickness"),
            get: |vc| vc.ssr.thickness,
            set: set_thickness,
        },
    ],
};
