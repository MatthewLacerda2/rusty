use super::*;
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_core::settings::quality::QualityPresets;

#[test]
fn game_time_rides_in_camera_pos_w() {
    let p = frame_params(2.5, Mat4::IDENTITY, [1.0; 3]);
    assert_eq!(p.camera_pos, [1.0, 1.0, 1.0, 2.5]);
}

#[test]
fn effect_runs_needs_camera_and_option() {
    let ssr = crate::postfx::effects::ssr::EFFECT.gate;
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

#[test]
fn every_effect_runs_on_every_preset() {
    use crate::postfx::effects::POST_EFFECTS;
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
