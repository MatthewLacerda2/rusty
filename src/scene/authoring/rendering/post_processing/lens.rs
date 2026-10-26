//! The lens and film effects' ops (#1439): Vignette, Chromatic Aberration and Film
//! Grain, each with its `active` flag and Unity's parameters, clamped to the ranges
//! Unity's volume overrides take. Split out of the post-processing ops to keep them
//! under the size cap; the same card and `Graphics.*` bindings call them.

use crate::components::{PostProcessingComponent, Vignette};
use crate::scene::authoring::sanitize::{clamped, finite_array_or, unit};

/// Set whether the vignette draws.
pub fn set_vignette_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.vignette.active = active;
}

/// Set how strongly the vignette darkens the edges, clamped to `0..=1`.
pub fn set_vignette_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.vignette.intensity = unit(value);
}

/// Set the vignette's falloff softness, clamped to `0.01..=1`.
pub fn set_vignette_smoothness(vc: &mut PostProcessingComponent, value: f32) {
    vc.vignette.smoothness = clamped(value, Vignette::MIN_SMOOTHNESS, 1.0);
}

/// Set the colour the vignette fades to, each channel clamped to `0..=1` (a colour
/// with a NaN or infinite channel keeps the current one).
pub fn set_vignette_color(vc: &mut PostProcessingComponent, color: [f32; 3]) {
    vc.vignette.color = finite_array_or(color, vc.vignette.color).map(unit);
}

/// Set the vignette's clear centre in screen uv, each axis clamped to `0..=1` (a
/// point with a NaN or infinite axis keeps the current one).
pub fn set_vignette_center(vc: &mut PostProcessingComponent, center: [f32; 2]) {
    vc.vignette.center = finite_array_or(center, vc.vignette.center).map(unit);
}

/// Set whether chromatic aberration draws.
pub fn set_chromatic_aberration_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.chromatic_aberration.active = active;
}

/// Set how far chromatic aberration splits the channels, clamped to `0..=1`.
pub fn set_chromatic_aberration_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.chromatic_aberration.intensity = unit(value);
}

/// Set whether film grain draws.
pub fn set_film_grain_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.film_grain.active = active;
}

/// Set the film grain's strength, clamped to `0..=1`.
pub fn set_film_grain_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.film_grain.intensity = unit(value);
}

/// Set how much the film grain fades out in bright areas, clamped to `0..=1`.
pub fn set_film_grain_response(vc: &mut PostProcessingComponent, value: f32) {
    vc.film_grain.response = unit(value);
}
