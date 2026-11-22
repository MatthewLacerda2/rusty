use super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use rusty_components::{PostProcessingComponent, Ssr};
use rusty_core::settings::quality::EffectQuality;
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

pub const EFFECT: Effect = Effect {
    name: "ssr",
    field: effect!(ssr, |c, v: Ssr| {
        set_active(c, v.active);
        set_quality(c, v.quality);
        set_steps(c, v.steps);
        set_step_length(c, v.step_length);
        set_min_smoothness(c, v.min_smoothness);
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
    ],
};
