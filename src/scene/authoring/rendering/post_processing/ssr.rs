//! Screen-space reflections' ops: the switch, the quality level (#1522) and the
//! `Custom` march, its step count and its step length (a knob: a count is no
//! tween).

use super::{Effect, Knob};
use crate::components::{PostProcessingComponent, Ssr};
use crate::core::settings::quality::EffectQuality;
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::clamped;

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

pub const EFFECT: Effect = Effect {
    name: "ssr",
    field: effect!(ssr, |c, v: Ssr| {
        set_active(c, v.active);
        set_quality(c, v.quality);
        set_steps(c, v.steps);
        set_step_length(c, v.step_length);
    }),
    knobs: &[Knob {
        field: "step_length",
        api: Some("SsrStepLength"),
        get: |vc| vc.ssr.step_length,
        set: set_step_length,
    }],
};
