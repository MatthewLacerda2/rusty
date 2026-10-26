//! src/scene/authoring/rendering/post_processing.rs — Shared post-processing ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `PostProcessingComponent` field by field: the master `active` flag, bloom
//! (active / intensity / threshold), color grading (exposure / contrast / saturation
//! / gamma / tonemap, and #1440's white balance / Lift Gamma Gain / hue shift), SSR (active / quality / temporal upsampling), shadows and SSAO
//! (active / radius / intensity, #436), the authored post-FX list (#397) and its
//! runtime params (#671), including the
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
    PostProcessingComponent, ShadowSettings, SsaoSettings, Tonemap, Wheel, WhiteBalance,
};
use crate::scene::authoring::sanitize::{
    at_least, clamped, finite_array_or, finite_or, finite_values, non_negative, unit,
};
use crate::shadergen::post_params::PostChain;

/// Set the master post-processing `active` flag.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.active = active;
}

/// Set whether bloom is active.
pub fn set_bloom_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.bloom_active = active;
}

/// Set the bloom intensity, clamped to `≥ 0`.
pub fn set_bloom_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom_intensity = non_negative(value);
}

/// Set the bloom threshold, clamped to `≥ 0`.
pub fn set_bloom_threshold(vc: &mut PostProcessingComponent, value: f32) {
    vc.bloom_threshold = non_negative(value);
}

/// Set the exposure (EV).
pub fn set_exposure(vc: &mut PostProcessingComponent, value: f32) {
    vc.exposure = finite_or(value, vc.exposure);
}

/// Set the contrast.
pub fn set_contrast(vc: &mut PostProcessingComponent, value: f32) {
    vc.contrast = finite_or(value, vc.contrast);
}

/// Set the saturation.
pub fn set_saturation(vc: &mut PostProcessingComponent, value: f32) {
    vc.saturation = finite_or(value, vc.saturation);
}

/// Set the display-gamma tweak (neutral `1.0`), clamped to `≥ 0.01` (avoids a
/// divide-by-zero in the shader's `c^(1/gamma)`).
pub fn set_gamma(vc: &mut PostProcessingComponent, value: f32) {
    vc.gamma = at_least(value, 0.01);
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
    vc.hue_shift = clamped(degrees, -180.0, 180.0);
}

/// Set the tonemap operator (typed — string parsing stays in the API adapter).
pub fn set_tonemap(vc: &mut PostProcessingComponent, tonemap: Tonemap) {
    vc.tonemap = tonemap;
}

/// Set whether SSR is active.
pub fn set_ssr_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssr_active = active;
}

/// Set the SSR quality preset name (`"Low"`/`"Medium"`/`"High"`/`"Ultra"`).
pub fn set_ssr_quality(vc: &mut PostProcessingComponent, quality: String) {
    vc.ssr_quality = quality;
}

/// Set whether SSR temporal upsampling is on.
pub fn set_ssr_temporal_upsampling(vc: &mut PostProcessingComponent, on: bool) {
    vc.ssr_temporal_upsampling = on;
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
    vc.custom_effects = names;
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
    set_post_param_with(vc, &PostChain::load(&vc.custom_effects), name, value)
}

/// [`set_post_param`] against an already-read `chain` of the volume's effects — a
/// tween (#661) resolves it once at start instead of reading sidecars every tick.
pub fn set_post_param_with(
    vc: &mut PostProcessingComponent,
    chain: &PostChain,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    chain.set(&mut vc.post_params, name, finite_values(value)?)
}

/// The value runtime param `name` of the volume's effects draws with: the stored one,
/// else the effect's baked default. Strict like [`set_post_param`].
pub fn post_param(vc: &PostProcessingComponent, name: &str) -> Result<Vec<f32>, String> {
    post_param_with(vc, &PostChain::load(&vc.custom_effects), name)
}

/// [`post_param`] against an already-read `chain`.
pub fn post_param_with(
    vc: &PostProcessingComponent,
    chain: &PostChain,
    name: &str,
) -> Result<Vec<f32>, String> {
    chain.get(&vc.post_params, name)
}

/// A bare module file stem: non-empty, no path separator, not hidden or `..`.
fn is_module_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::PostProcessingComponent;
    use crate::scene::Scene;

    fn scene_with_vc() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Volume".to_string());
        let c = PostProcessingComponent {
            bloom_active: false,
            bloom_threshold: 1.0,
            ssr_active: false,
            ssr_quality: "Medium".to_string(),
            ssr_temporal_upsampling: false,
            ..crate::scene::authoring::default_post_processing()
        };
        scene.world.set_post_processing(id, Some(c));
        (scene, id)
    }

    #[test]
    fn ops_write_through_with_clamps() {
        let (mut scene, id) = scene_with_vc();
        let mut e = scene.world.post_processing_mut(id).unwrap();
        set_bloom_intensity(&mut e, -3.0);
        set_bloom_threshold(&mut e, -1.0);
        set_gamma(&mut e, -5.0);
        set_tonemap(&mut e, Tonemap::Reinhard);
        set_ssr_quality(&mut e, "High".to_string());
        set_ssr_temporal_upsampling(&mut e, true);
        set_shadow_cascades(&mut e, 9);
        set_shadow_distance(&mut e, -4.0);
        set_ssao_active(&mut e, false);
        set_ssao_radius(&mut e, -1.0);
        set_ssao_intensity(&mut e, 9.0);
        set_white_balance(&mut e, 250.0, f32::NAN);
        set_wheel(&mut e, Wheel::Gain, [2.0, 0.5, -1.0], -3.0);
        set_hue_shift(&mut e, 400.0);
        let vc = &*e;
        assert_eq!(vc.bloom_intensity, 0.0);
        assert_eq!(vc.bloom_threshold, 0.0);
        assert_eq!(vc.gamma, 0.01, "gamma clamps to its floor");
        assert_eq!(vc.tonemap, Tonemap::Reinhard);
        assert_eq!(vc.ssr_quality, "High");
        assert!(vc.ssr_temporal_upsampling);
        assert_eq!(vc.shadows.cascades, 4, "cascades clamp to the map's layers");
        assert_eq!(
            vc.shadows.distance, 1.0,
            "shadow distance clamps to its floor"
        );
        assert!(!vc.ssao.active);
        assert_eq!(vc.ssao.radius, 0.01, "radius clamps to its floor");
        assert_eq!(vc.ssao.intensity, 4.0, "intensity clamps to its ceiling");
        let wb = vc.white_balance;
        assert_eq!(
            (wb.temperature, wb.tint),
            (100.0, 0.0),
            "NaN tint is neutral"
        );
        let gain = vc.lift_gamma_gain.gain;
        assert_eq!((gain.color, gain.offset), ([1.0, 0.5, 0.0], -1.0));
        assert_eq!(vc.hue_shift, 180.0);
    }

    #[test]
    fn custom_effects_keep_order_and_refuse_paths() {
        let (mut scene, id) = scene_with_vc();
        let mut e = scene.world.post_processing_mut(id).unwrap();
        set_custom_effects(&mut e, vec!["crt".into(), "grain".into()]).unwrap();
        for bad in ["", "../etc", "a/b", "a\\b", ".hidden"] {
            let err = set_custom_effects(&mut e, vec!["ok".into(), bad.into()]).unwrap_err();
            assert!(err.contains(&format!("{bad:?}")), "{err}");
        }
        assert_eq!(
            e.custom_effects,
            ["crt", "grain"],
            "a refused write changes nothing"
        );
    }
}
