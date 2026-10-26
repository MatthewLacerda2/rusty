//! The volume's lighting settings' ops (#435, #436): the sun's shadow cascades and
//! reach, and screen-space ambient occlusion. No post effects, so no knobs: their
//! rows sit beside the effects' in [`super::FIELDS`].

use crate::components::{PostProcessingComponent, ShadowSettings, SsaoSettings};
use crate::scene::authoring::fields::table::{op, with, Field};
use crate::scene::authoring::sanitize::{at_least, clamped};

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

/// The field-table rows of the volume's own settings: its switch, its shadows and
/// its ambient occlusion.
pub const FIELDS: [Field<PostProcessingComponent>; 3] = [
    op!("active", super::set_active),
    with!("shadows", |c, v: ShadowSettings| {
        set_shadow_cascades(c, v.cascades);
        set_shadow_distance(c, v.distance);
    }),
    with!("ssao", |c, v: SsaoSettings| {
        set_ssao_active(c, v.active);
        set_ssao_radius(c, v.radius);
        set_ssao_intensity(c, v.intensity);
    }),
];
