//! The frame every post pass shares, and the one gate every effect asks.

use super::*;
use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use crate::core::settings::quality::QualityPresets;

#[test]
fn game_time_rides_in_camera_pos_w() {
    let mut scene = Scene::new();
    scene.shader_time = 2.5;
    let p = frame_params(&scene, Mat4::IDENTITY, [1.0; 3]);
    assert_eq!(p.camera_pos, [1.0, 1.0, 1.0, 2.5]);
}

/// An option allows; it never turns on what the camera left off.
#[test]
fn effect_runs_needs_camera_and_option() {
    let ssr = crate::render::postfx::effects::ssr::EFFECT.gate;
    let all = GraphicsSettings::default();
    assert!(effect_runs(all, ssr, true));
    assert!(!effect_runs(all, ssr, false), "camera off");
    let mut options = GraphicsOptions::default();
    options.set(GraphicsOption::Ssr, false);
    let off = GraphicsSettings {
        options,
        ..Default::default()
    };
    assert!(!effect_runs(off, ssr, true), "option off");
}

/// No quality preset gates an effect (#1522): every effect a camera has on runs on
/// every seeded preset.
#[test]
fn every_effect_runs_on_every_preset() {
    use crate::render::postfx::effects::POST_EFFECTS;
    for preset in &QualityPresets::default().presets {
        let graphics = GraphicsSettings {
            quality: preset.values,
            ..Default::default()
        };
        for effect in POST_EFFECTS {
            assert!(effect_runs(graphics, effect.gate, true), "{}", effect.name);
        }
    }
}
