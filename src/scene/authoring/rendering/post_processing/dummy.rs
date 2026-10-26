//! The test-only dummy effect's ops (#1467): its switch and its one knob.

use super::{Effect, Knob};
use crate::components::rendering::post_processing::dummy::Dummy;
use crate::components::PostProcessingComponent;
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::unit;

/// Set whether the dummy runs.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.dummy.active = active;
}

/// Set the dummy's strength, clamped to `0..=1`.
pub fn set_strength(vc: &mut PostProcessingComponent, value: f32) {
    vc.dummy.strength = unit(value);
}

pub const EFFECT: Effect = Effect {
    name: "dummy",
    field: effect!(dummy, |c, v: Dummy| {
        set_active(c, v.active);
        set_strength(c, v.strength);
    }),
    knobs: &[Knob {
        field: "strength",
        api: Some("DummyStrength"),
        get: |vc| vc.dummy.strength,
        set: set_strength,
    }],
};
