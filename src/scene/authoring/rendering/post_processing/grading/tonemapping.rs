//! Tonemapping's ops: the operator (typed: the `Graphics` adapter parses its name)
//! and the display-gamma tweak.

use super::super::{Effect, Knob};
use crate::components::{PostProcessingComponent, Tonemap, Tonemapping};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::at_least;

/// Set the tonemap operator.
pub fn set_mode(vc: &mut PostProcessingComponent, tonemap: Tonemap) {
    vc.tonemapping.mode = tonemap;
}

/// Set the display-gamma tweak (neutral `1.0`), clamped to `≥ 0.01` (avoids a
/// divide-by-zero in the shader's `c^(1/gamma)`).
pub fn set_display_gamma(vc: &mut PostProcessingComponent, value: f32) {
    vc.tonemapping.display_gamma = at_least(value, 0.01);
}

pub const EFFECT: Effect = Effect {
    name: "tonemapping",
    field: effect!(tonemapping, |c, v: Tonemapping| {
        set_mode(c, v.mode);
        set_display_gamma(c, v.display_gamma);
    }),
    knobs: &[Knob {
        field: "display_gamma",
        api: Some("Gamma"),
        get: |vc| vc.tonemapping.display_gamma,
        set: set_display_gamma,
    }],
};
