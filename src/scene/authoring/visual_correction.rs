//! src/scene/authoring/visual_correction.rs — Shared visual-correction ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `VisualCorrectionComponent` field by field: the master `active` flag, bloom
//! (active / intensity / threshold), color grading (exposure / contrast / saturation
//! / gamma / tonemap), and SSR (active / quality / temporal upsampling), including the
//! validation each write owns (the `≥ 0` clamps on bloom intensity/threshold, the
//! `≥ 0.01` clamp on gamma).
//!
//! BOTH the editor's Visual Correction card and the Lua `Graphics.*` setters route
//! through these, so the egui panel and the binding share one write + one clamp
//! (#287). The `Graphics` adapter still parses the tonemap/quality strings into the
//! typed values these ops take.
//!
//! Allowed deps: components (the `VisualCorrectionComponent`/`Tonemap` data). Pure.

use crate::components::{Tonemap, VisualCorrectionComponent};

/// Set the master visual-correction `active` flag.
pub fn set_active(vc: &mut VisualCorrectionComponent, active: bool) {
    vc.active = active;
}

/// Set whether bloom is active.
pub fn set_bloom_active(vc: &mut VisualCorrectionComponent, active: bool) {
    vc.bloom_active = active;
}

/// Set the bloom intensity, clamped to `≥ 0`.
pub fn set_bloom_intensity(vc: &mut VisualCorrectionComponent, value: f32) {
    vc.bloom_intensity = value.max(0.0);
}

/// Set the bloom threshold, clamped to `≥ 0`.
pub fn set_bloom_threshold(vc: &mut VisualCorrectionComponent, value: f32) {
    vc.bloom_threshold = value.max(0.0);
}

/// Set the exposure (EV).
pub fn set_exposure(vc: &mut VisualCorrectionComponent, value: f32) {
    vc.exposure = value;
}

/// Set the contrast.
pub fn set_contrast(vc: &mut VisualCorrectionComponent, value: f32) {
    vc.contrast = value;
}

/// Set the saturation.
pub fn set_saturation(vc: &mut VisualCorrectionComponent, value: f32) {
    vc.saturation = value;
}

/// Set the output gamma, clamped to `≥ 0.01` (avoids a divide-by-zero encode).
pub fn set_gamma(vc: &mut VisualCorrectionComponent, value: f32) {
    vc.gamma = value.max(0.01);
}

/// Set the tonemap operator (typed — string parsing stays in the API adapter).
pub fn set_tonemap(vc: &mut VisualCorrectionComponent, tonemap: Tonemap) {
    vc.tonemap = tonemap;
}

/// Set whether SSR is active.
pub fn set_ssr_active(vc: &mut VisualCorrectionComponent, active: bool) {
    vc.ssr_active = active;
}

/// Set the SSR quality preset name (`"Low"`/`"Medium"`/`"High"`/`"Ultra"`).
pub fn set_ssr_quality(vc: &mut VisualCorrectionComponent, quality: String) {
    vc.ssr_quality = quality;
}

/// Set whether SSR temporal upsampling is on.
pub fn set_ssr_temporal_upsampling(vc: &mut VisualCorrectionComponent, on: bool) {
    vc.ssr_temporal_upsampling = on;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::VisualCorrectionComponent;
    use crate::scene::Scene;

    fn scene_with_vc() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Volume".to_string());
        let c = VisualCorrectionComponent {
            active: true,
            bloom_active: false,
            bloom_intensity: 1.0,
            bloom_threshold: 1.0,
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            ssr_active: false,
            ssr_quality: "Medium".to_string(),
            ssr_temporal_upsampling: false,
            tonemap: Tonemap::Aces,
            gamma: 2.2,
        };
        scene.world.set_visual_correction(id, Some(c));
        (scene, id)
    }

    #[test]
    fn ops_write_through_with_clamps() {
        let (mut scene, id) = scene_with_vc();
        let mut e = scene.world.visual_correction_mut(id).unwrap();
        set_bloom_intensity(&mut e, -3.0);
        set_bloom_threshold(&mut e, -1.0);
        set_gamma(&mut e, -5.0);
        set_tonemap(&mut e, Tonemap::Reinhard);
        set_ssr_quality(&mut e, "High".to_string());
        set_ssr_temporal_upsampling(&mut e, true);
        let vc = &*e;
        assert_eq!(vc.bloom_intensity, 0.0);
        assert_eq!(vc.bloom_threshold, 0.0);
        assert_eq!(vc.gamma, 0.01, "gamma clamps to its floor");
        assert_eq!(vc.tonemap, Tonemap::Reinhard);
        assert_eq!(vc.ssr_quality, "High");
        assert!(vc.ssr_temporal_upsampling);
    }
}
