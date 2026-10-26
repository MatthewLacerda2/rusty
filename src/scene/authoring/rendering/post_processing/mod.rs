//! src/scene/authoring/rendering/post_processing/ — Shared post-processing ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `PostProcessingComponent` field by field: the master `active` flag, bloom
//! (active / intensity / threshold), color grading (exposure / contrast / saturation
//! / gamma / tonemap, #1440's white balance / Lift Gamma Gain / hue shift, and
//! #1441's Color Lookup), SSR (active / quality / temporal upsampling), shadows and SSAO
//! (active / radius / intensity, #436), the authored post-FX list (#397) and its
//! runtime params (#671), the lens effects (#1439, in `lens`) and the depth of field
//! (#1445, in `depth_of_field`), including the
//! validation each write owns (the `≥ 0` clamps on bloom intensity/threshold, the
//! `≥ 0.01` clamp on gamma).
//!
//! BOTH the editor's Post Processing card and the Lua `Graphics.*` setters route
//! through these, so the egui panel and the binding share one write + one clamp
//! (#287). The `Graphics` adapter still parses the tonemap/quality strings into the
//! typed values these ops take.
//!
//! Pure.

use crate::components::{
    AutoExposure, ExposureMode, PostProcessingComponent, ShadowSettings, SsaoSettings, Tonemap,
    Wheel, WhiteBalance,
};
use crate::core::paths::stored;
use crate::scene::authoring::sanitize::{
    at_least, clamped, finite_array_or, finite_or, finite_values, non_negative, unit,
};
use crate::shadergen::post_params::PostChain;

mod lens;

pub use lens::{
    set_chromatic_aberration_active, set_chromatic_aberration_intensity, set_film_grain_active,
    set_film_grain_intensity, set_film_grain_response, set_vignette_active, set_vignette_center,
    set_vignette_color, set_vignette_intensity, set_vignette_smoothness,
};

mod depth_of_field;

pub use depth_of_field::{
    dof_knob, set_dof_aperture, set_dof_focal_length, set_dof_focus_distance, set_dof_gaussian_end,
    set_dof_gaussian_max_radius, set_dof_gaussian_start, set_dof_mode, DofKnob, DOF_KNOBS,
};

/// Set the master post-processing `active` flag.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.active = active;
}

/// Set whether bloom is active.
pub fn set_bloom_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.bloom.active = active;
}

/// Set the bloom intensity, clamped to `≥ 0`.
pub fn set_bloom_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.intensity = non_negative(value);
}

/// Set the bloom threshold, clamped to `≥ 0`.
pub fn set_bloom_threshold(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom.threshold = non_negative(value);
}

/// Set the exposure (EV).
pub fn set_exposure(vc: &mut PostProcessingComponent, value: f32) {
    vc.exposure.fixed_ev = finite_or(value, vc.exposure.fixed_ev);
}

/// Set where the exposure comes from (#1444): the fixed `exposure`, or metered.
pub fn set_exposure_mode(vc: &mut PostProcessingComponent, mode: ExposureMode) {
    vc.exposure.mode = mode;
}

/// Set the automatic exposure's limits (#1444), each clamped to
/// `±AutoExposure::EV_RANGE`; `max_ev` is floored at `min_ev`.
pub fn set_exposure_limits(vc: &mut PostProcessingComponent, min_ev: f32, max_ev: f32) {
    let r = AutoExposure::EV_RANGE;
    let min_ev = clamped(min_ev, -r, r);
    vc.exposure.automatic.min_ev = min_ev;
    vc.exposure.automatic.max_ev = clamped(max_ev, min_ev, r);
}

/// Set the automatic exposure's compensation in EV (#1444), clamped to
/// `±AutoExposure::EV_RANGE`.
pub fn set_exposure_compensation(vc: &mut PostProcessingComponent, ev: f32) {
    let r = AutoExposure::EV_RANGE;
    vc.exposure.automatic.compensation = clamped(ev, -r, r);
}

/// Set how fast the automatic exposure rises and falls (#1444), in EV per second,
/// each clamped to `0..=AutoExposure::MAX_SPEED`.
pub fn set_adaptation_speed(vc: &mut PostProcessingComponent, up: f32, down: f32) {
    let max = AutoExposure::MAX_SPEED;
    vc.exposure.automatic.speed_up = clamped(up, 0.0, max);
    vc.exposure.automatic.speed_down = clamped(down, 0.0, max);
}

/// Set the histogram window the meter averages (#1444): `low` clamped to
/// `0..=100`, `high` to `low..=100`.
pub fn set_histogram_percent(vc: &mut PostProcessingComponent, low: f32, high: f32) {
    let low = clamped(low, 0.0, 100.0);
    vc.exposure.automatic.low_percent = low;
    vc.exposure.automatic.high_percent = clamped(high, low, 100.0);
}

/// Set the contrast.
pub fn set_contrast(vc: &mut PostProcessingComponent, value: f32) {
    vc.color_adjustments.contrast = finite_or(value, vc.color_adjustments.contrast);
}

/// Set the saturation.
pub fn set_saturation(vc: &mut PostProcessingComponent, value: f32) {
    vc.color_adjustments.saturation = finite_or(value, vc.color_adjustments.saturation);
}

/// Set the display-gamma tweak (neutral `1.0`), clamped to `≥ 0.01` (avoids a
/// divide-by-zero in the shader's `c^(1/gamma)`).
pub fn set_gamma(vc: &mut PostProcessingComponent, value: f32) {
    vc.tonemapping.display_gamma = at_least(value, 0.01);
}

/// Set the white balance (#1440): `temperature` and `tint`, each clamped to
/// `-100..=100`.
pub fn set_white_balance(vc: &mut PostProcessingComponent, temperature: f32, tint: f32) {
    let r = WhiteBalance::RANGE;
    vc.white_balance = WhiteBalance {
        temperature: clamped(temperature, -r, r),
        tint: clamped(tint, -r, r),
    };
}

/// Set one Lift / Gamma / Gain wheel (#1440): its colour, each channel clamped to
/// `0..=1` (a colour with a NaN or infinite channel keeps the current one), and its
/// offset, clamped to `-1..=1`.
pub fn set_wheel(vc: &mut PostProcessingComponent, which: Wheel, color: [f32; 3], offset: f32) {
    let wheel = vc.lift_gamma_gain.wheel_mut(which);
    wheel.color = finite_array_or(color, wheel.color).map(unit);
    wheel.offset = clamped(offset, -1.0, 1.0);
}

/// Set the hue shift in degrees (#1440), clamped to `-180..=180`.
pub fn set_hue_shift(vc: &mut PostProcessingComponent, degrees: f32) {
    vc.color_adjustments.hue_shift = clamped(degrees, -180.0, 180.0);
}

/// Set the Color Lookup's LUT file (#1441) as a [`stored`] path; `None` or a blank
/// one clears it. The renderer logs once and grades neutral when it is missing or
/// malformed.
pub fn set_lut(vc: &mut PostProcessingComponent, path: Option<String>) {
    vc.color_lookup.lut = path.map(|p| stored(&p)).filter(|p| !p.is_empty());
}

/// Set how much of the Color Lookup applies (#1441), clamped to `0..=1`.
pub fn set_lut_contribution(vc: &mut PostProcessingComponent, value: f32) {
    vc.color_lookup.contribution = unit(value);
}

/// Set the tonemap operator (typed — string parsing stays in the API adapter).
pub fn set_tonemap(vc: &mut PostProcessingComponent, tonemap: Tonemap) {
    vc.tonemapping.mode = tonemap;
}

/// Set whether SSR is active.
pub fn set_ssr_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssr.active = active;
}

/// Set the SSR quality preset name (`"Low"`/`"Medium"`/`"High"`/`"Ultra"`).
pub fn set_ssr_quality(vc: &mut PostProcessingComponent, quality: String) {
    vc.ssr.quality = quality;
}

/// Set whether SSR temporal upsampling is on.
pub fn set_ssr_temporal_upsampling(vc: &mut PostProcessingComponent, on: bool) {
    vc.ssr.temporal_upsampling = on;
}

/// Set how many shadow cascades split the view, clamped to `1..=4`.
pub fn set_shadow_cascades(vc: &mut PostProcessingComponent, count: u32) {
    vc.shadows.cascades = count.clamp(1, ShadowSettings::MAX_CASCADES);
}

/// Set how far from the camera sun shadows reach, clamped to `≥ 1` world unit.
pub fn set_shadow_distance(vc: &mut PostProcessingComponent, distance: f32) {
    vc.shadows.distance = at_least(distance, ShadowSettings::MIN_DISTANCE);
}

/// Set whether screen-space ambient occlusion runs (#436).
pub fn set_ssao_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssao.active = active;
}

/// Set the AO search radius in world units, clamped to `≥ 0.01`.
pub fn set_ssao_radius(vc: &mut PostProcessingComponent, radius: f32) {
    vc.ssao.radius = at_least(radius, SsaoSettings::MIN_RADIUS);
}

/// Set the AO strength, clamped to `0..=4`.
pub fn set_ssao_intensity(vc: &mut PostProcessingComponent, intensity: f32) {
    vc.ssao.intensity = clamped(intensity, 0.0, SsaoSettings::MAX_INTENSITY);
}

/// Replace the ordered list of authored postfx modules the volume runs (#397).
///
/// Each entry is a module name as `Shader.Bake` wrote it (`<name>.wgsl`, no
/// extension). A name that is empty or reaches outside the shader workspace (a path
/// separator, a leading `.`) is refused by name and nothing is written; a module
/// that simply isn't baked yet is accepted — the renderer skips it until it is.
pub fn set_custom_effects(
    vc: &mut PostProcessingComponent,
    names: Vec<String>,
) -> Result<(), String> {
    if let Some(bad) = names.iter().find(|n| !is_module_name(n)) {
        return Err(format!(
            "invalid post-FX effect name {bad:?}: want a baked module name, not a path"
        ));
    }
    vc.custom_effects.names = names;
    Ok(())
}

/// Set runtime param `name` of the volume's authored effects (#671), checked against
/// the layouts of the modules in `custom_effects`: a name none of them exposes at
/// runtime, or a wrong number of values, is an error naming it and nothing is
/// written. One number broadcasts to every lane. A buffer write for the renderer,
/// never a re-bake.
pub fn set_post_param(
    vc: &mut PostProcessingComponent,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    set_post_param_with(vc, &PostChain::load(&vc.custom_effects.names), name, value)
}

/// [`set_post_param`] against an already-read `chain` of the volume's effects — a
/// tween (#661) resolves it once at start instead of reading sidecars every tick.
pub fn set_post_param_with(
    vc: &mut PostProcessingComponent,
    chain: &PostChain,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    chain.set(&mut vc.custom_effects.params, name, finite_values(value)?)
}

/// The value runtime param `name` of the volume's effects draws with: the stored one,
/// else the effect's baked default. Strict like [`set_post_param`].
pub fn post_param(vc: &PostProcessingComponent, name: &str) -> Result<Vec<f32>, String> {
    post_param_with(vc, &PostChain::load(&vc.custom_effects.names), name)
}

/// [`post_param`] against an already-read `chain`.
pub fn post_param_with(
    vc: &PostProcessingComponent,
    chain: &PostChain,
    name: &str,
) -> Result<Vec<f32>, String> {
    chain.get(&vc.custom_effects.params, name)
}

/// A bare module file stem: non-empty, no path separator, not hidden or `..`.
fn is_module_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

#[cfg(test)]
mod tests;
