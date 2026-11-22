use super::super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use rusty_components::{ColorLookup, PostProcessingComponent};
use rusty_core::paths::stored;
use rusty_scene::rules::sanitize::unit;

pub fn set_lut(vc: &mut PostProcessingComponent, path: Option<String>) {
    vc.color_lookup.lut = path.map(|p| stored(&p)).filter(|p| !p.is_empty());
}

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
