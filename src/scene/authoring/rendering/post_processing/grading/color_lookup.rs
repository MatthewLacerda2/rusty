//! The Color Lookup's ops (#1441): the LUT file and its contribution.

use super::super::{Effect, Knob};
use crate::components::{ColorLookup, PostProcessingComponent};
use crate::core::paths::stored;
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::unit;

/// Set the LUT file as a [`stored`] path; `None` or a blank one clears it. The
/// renderer logs once and grades neutral when it is missing or malformed.
pub fn set_lut(vc: &mut PostProcessingComponent, path: Option<String>) {
    vc.color_lookup.lut = path.map(|p| stored(&p)).filter(|p| !p.is_empty());
}

/// Set how much of the LUT applies, clamped to `0..=1`.
pub fn set_contribution(vc: &mut PostProcessingComponent, value: f32) {
    vc.color_lookup.contribution = unit(value);
}

pub const EFFECT: Effect = Effect {
    name: "color_lookup",
    field: effect!(color_lookup, |c, v: ColorLookup| {
        set_lut(c, v.lut);
        set_contribution(c, v.contribution);
    }),
    knobs: &[Knob {
        field: "contribution",
        api: Some("ColorLookupContribution"),
        get: |vc| vc.color_lookup.contribution,
        set: set_contribution,
    }],
};
