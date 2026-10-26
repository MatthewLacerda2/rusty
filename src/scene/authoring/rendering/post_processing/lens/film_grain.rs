//! Film grain's ops: the switch, the strength and the highlight response.

use super::super::{Effect, Knob};
use crate::components::{FilmGrain, PostProcessingComponent};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::unit;

/// Set whether film grain draws.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.film_grain.active = active;
}

/// Set the film grain's strength, clamped to `0..=1`.
pub fn set_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.film_grain.intensity = unit(value);
}

/// Set how much the film grain fades out in bright areas, clamped to `0..=1`.
pub fn set_response(vc: &mut PostProcessingComponent, value: f32) {
    vc.film_grain.response = unit(value);
}

pub const EFFECT: Effect = Effect {
    name: "film_grain",
    field: effect!(film_grain, |c, v: FilmGrain| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
        set_response(c, v.response);
    }),
    knobs: &[
        Knob {
            field: "intensity",
            api: Some("FilmGrainIntensity"),
            get: |vc| vc.film_grain.intensity,
            set: set_intensity,
        },
        Knob {
            field: "response",
            api: Some("FilmGrainResponse"),
            get: |vc| vc.film_grain.response,
            set: set_response,
        },
    ],
};
