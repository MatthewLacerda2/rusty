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
    let all = GraphicsSettings::from(QualityPreset::High);
    assert!(effect_runs(all, GraphicsOption::Ssr, true));
    assert!(!effect_runs(all, GraphicsOption::Ssr, false), "camera off");
    let medium = GraphicsSettings::from(QualityPreset::Medium);
    assert!(!effect_runs(medium, GraphicsOption::Ssr, true), "tier off");
    let mut options = GraphicsOptions::default();
    options.set(GraphicsOption::Ssr, false);
    let off = GraphicsSettings {
        quality: QualityPreset::High,
        options,
    };
    assert!(!effect_runs(off, GraphicsOption::Ssr, true), "option off");
}
