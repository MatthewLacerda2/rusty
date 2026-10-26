//! Chromatic aberration's ops: the switch and the strength.

use super::super::{Effect, Knob};
use crate::components::{ChromaticAberration, PostProcessingComponent};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::unit;

/// Set whether chromatic aberration draws.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.chromatic_aberration.active = active;
}

/// Set how far chromatic aberration splits the channels, clamped to `0..=1`.
pub fn set_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.chromatic_aberration.intensity = unit(value);
}

pub const EFFECT: Effect = Effect {
    name: "chromatic_aberration",
    field: effect!(chromatic_aberration, |c, v: ChromaticAberration| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
    }),
    knobs: &[Knob {
        field: "intensity",
        api: Some("ChromaticAberrationIntensity"),
        get: |vc| vc.chromatic_aberration.intensity,
        set: set_intensity,
    }],
};
