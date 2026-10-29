//! The volume's own settings' ops: its master switch, the sun's shadow cascades and
//! reach (#435) and screen-space ambient occlusion (#436), its quality level and
//! `Custom` samples and resolution (#1522). No post effects, so no
//! knobs: their rows sit before the effects' in [`super::FIELDS`]. Like the effects'
//! rows they merge: a partial write keeps the other fields, an unknown key errors.

use crate::components::{PostProcessingComponent, ShadowSettings, SsaoSettings};
use crate::core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use crate::scene::authoring::fields::table::{effect, op, Field};
use crate::scene::authoring::sanitize::{at_least, clamped};

/// Set the master post-processing `active` flag.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.active = active;
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

/// Set which samples and resolution AO runs at: a level of the active quality
/// preset, or `Custom`.
pub fn set_ssao_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.ssao.quality = quality;
}

/// Set the `Custom` sample count, clamped to `1..=64`.
pub fn set_ssao_samples(vc: &mut PostProcessingComponent, samples: u32) {
    vc.ssao.samples = samples.clamp(1, SsaoSettings::MAX_SAMPLES);
}

/// Set the `Custom` occlusion target's divisor of the view's size, clamped to
/// `1..=4`.
pub fn set_ssao_resolution_divisor(vc: &mut PostProcessingComponent, divisor: u32) {
    vc.ssao.resolution_divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR);
}

/// The field-table rows of the volume's own settings: its switch, its shadows and
/// its ambient occlusion.
pub const FIELDS: [Field<PostProcessingComponent>; 3] = [
    op!("active", set_active),
    effect!(shadows, |c, v: ShadowSettings| {
        set_shadow_cascades(c, v.cascades);
        set_shadow_distance(c, v.distance);
    }),
    effect!(ssao, |c, v: SsaoSettings| {
        set_ssao_active(c, v.active);
        set_ssao_radius(c, v.radius);
        set_ssao_intensity(c, v.intensity);
        set_ssao_quality(c, v.quality);
        set_ssao_samples(c, v.samples);
        set_ssao_resolution_divisor(c, v.resolution_divisor);
    }),
];
