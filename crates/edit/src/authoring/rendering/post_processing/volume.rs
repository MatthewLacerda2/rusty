use crate::authoring::fields::table::{effect, op, Field};
use rusty_components::{PostProcessingComponent, ShadowSettings, SsaoSettings};
use rusty_core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use rusty_scene::rules::sanitize::{at_least, clamped};

pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.active = active;
}

pub fn set_shadow_cascades(vc: &mut PostProcessingComponent, count: u32) {
    vc.shadows.cascades = count.clamp(1, ShadowSettings::MAX_CASCADES);
}

pub fn set_shadow_distance(vc: &mut PostProcessingComponent, distance: f32) {
    vc.shadows.distance = at_least(distance, ShadowSettings::MIN_DISTANCE);
}

pub fn set_ssao_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssao.active = active;
}

pub fn set_ssao_radius(vc: &mut PostProcessingComponent, radius: f32) {
    vc.ssao.radius = at_least(radius, SsaoSettings::MIN_RADIUS);
}

pub fn set_ssao_intensity(vc: &mut PostProcessingComponent, intensity: f32) {
    vc.ssao.intensity = clamped(intensity, 0.0, SsaoSettings::MAX_INTENSITY);
}

pub fn set_ssao_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.ssao.quality = quality;
}

pub fn set_ssao_samples(vc: &mut PostProcessingComponent, samples: u32) {
    vc.ssao.samples = samples.clamp(1, SsaoSettings::MAX_SAMPLES);
}

pub fn set_ssao_resolution_divisor(vc: &mut PostProcessingComponent, divisor: u32) {
    vc.ssao.resolution_divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR);
}

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
