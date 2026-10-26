//! Color adjustments' ops: contrast, saturation and the hue shift.

use super::super::{Effect, Knob};
use crate::components::{ColorAdjustments, PostProcessingComponent};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::{clamped, finite_or};

/// Set the contrast (a non-finite value keeps the current one).
pub fn set_contrast(vc: &mut PostProcessingComponent, value: f32) {
    let a = &mut vc.color_adjustments;
    a.contrast = finite_or(value, a.contrast);
}

/// Set the saturation (a non-finite value keeps the current one).
pub fn set_saturation(vc: &mut PostProcessingComponent, value: f32) {
    let a = &mut vc.color_adjustments;
    a.saturation = finite_or(value, a.saturation);
}

/// Set the hue shift in degrees, clamped to `-180..=180`.
pub fn set_hue_shift(vc: &mut PostProcessingComponent, degrees: f32) {
    vc.color_adjustments.hue_shift = clamped(degrees, -180.0, 180.0);
}

pub const EFFECT: Effect = Effect {
    name: "color_adjustments",
    field: effect!(color_adjustments, |c, v: ColorAdjustments| {
        set_contrast(c, v.contrast);
        set_saturation(c, v.saturation);
        set_hue_shift(c, v.hue_shift);
    }),
    knobs: &[
        Knob {
            field: "contrast",
            api: Some("Contrast"),
            get: |vc| vc.color_adjustments.contrast,
            set: set_contrast,
        },
        Knob {
            field: "saturation",
            api: Some("Saturation"),
            get: |vc| vc.color_adjustments.saturation,
            set: set_saturation,
        },
        Knob {
            field: "hue_shift",
            api: Some("HueShift"),
            get: |vc| vc.color_adjustments.hue_shift,
            set: set_hue_shift,
        },
    ],
};
