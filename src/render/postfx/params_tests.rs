//! The frame every post pass shares, and the one gate every effect asks.

use super::*;
use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use crate::core::settings::quality::QualityPreset;

#[test]
fn game_time_rides_in_camera_pos_w() {
    let mut scene = Scene::new();
    scene.shader_time = 2.5;
    let p = frame_params(&scene, Mat4::IDENTITY, [1.0; 3]);
    assert_eq!(p.camera_pos, [1.0, 1.0, 1.0, 2.5]);
}

/// An option allows; it never turns on what the camera or the tier left off.
#[test]
fn effect_runs_needs_camera_option_and_tier() {
    let ssr = crate::render::postfx::effects::ssr::EFFECT.gate;
    let all = GraphicsSettings::from(QualityPreset::High);
    assert!(effect_runs(all, ssr, true));
    assert!(!effect_runs(all, ssr, false), "camera off");
    let medium = GraphicsSettings::from(QualityPreset::Medium);
    assert!(!effect_runs(medium, ssr, true), "tier off");
    let mut options = GraphicsOptions::default();
    options.set(GraphicsOption::Ssr, false);
    let off = GraphicsSettings {
        quality: QualityPreset::High,
        options,
    };
    assert!(!effect_runs(off, ssr, true), "option off");
}

/// Each effect declares the tiers it runs on: SSR on High only, motion blur and
/// depth of field off on Low, FXAA and bloom on every tier.
#[test]
fn each_effect_runs_on_the_tiers_it_declares() {
    use crate::render::postfx::effects::{bloom, depth_of_field, fxaa, motion_blur, ssr};
    use QualityPreset::*;
    let runs = |gate: Gate, tier| effect_runs(GraphicsSettings::from(tier), gate, true);
    assert!(!runs(motion_blur::EFFECT.gate, Low) && runs(motion_blur::EFFECT.gate, Medium));
    assert!(!runs(depth_of_field::EFFECT.gate, Low));
    assert!(!runs(ssr::EFFECT.gate, Medium) && runs(ssr::EFFECT.gate, High));
    for tier in QualityPreset::ALL {
        assert!(
            runs(fxaa::EFFECT.gate, tier) && runs(bloom::EFFECT.gate, tier),
            "{tier}"
        );
    }
}
