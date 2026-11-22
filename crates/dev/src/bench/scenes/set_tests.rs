use rusty_core::settings::quality::{QualityPresets, ShadowmaskMode};

use super::*;

fn set(pairs: &[&str]) -> Result<Overrides, String> {
    let pairs: Vec<String> = pairs.iter().map(|p| p.to_string()).collect();
    Overrides::parse(&pairs)
}

#[test]
fn a_known_key_changes_the_running_preset_and_only_it() {
    let mut presets = QualityPresets::default();
    let before = presets.clone();
    let o = set(&[
        "gpu_occlusion_culling=true",
        "shadowmask_mode=DistanceShadowmask",
    ])
    .unwrap();
    assert_eq!(
        o.said,
        [
            "gpu_occlusion_culling=true",
            "shadowmask_mode=DistanceShadowmask"
        ]
    );
    o.apply(&mut presets).unwrap();
    let running = presets.values(None);
    assert!(running.gpu_occlusion_culling);
    assert_eq!(running.shadowmask_mode, ShadowmaskMode::DistanceShadowmask);
    assert_eq!(
        presets.get("Low"),
        before.get("Low"),
        "only the running preset"
    );
    assert_eq!(presets.names(), before.names());
}

#[test]
fn a_nested_key_sets_one_level_of_one_effect() {
    let mut presets = QualityPresets::default();
    let high = presets.values(None).motion_blur_samples.high;
    let o = set(&["motion_blur_samples.low=3"]).unwrap();
    o.apply(&mut presets).unwrap();
    let blur = presets.values(None).motion_blur_samples;
    assert_eq!((blur.low, blur.high), (3, high));
}

#[test]
fn a_project_with_no_preset_runs_the_engine_values_overridden() {
    let mut presets = QualityPresets {
        default: None,
        presets: Vec::new(),
    };
    set(&["gpu_cluster_culling=true"])
        .unwrap()
        .apply(&mut presets)
        .unwrap();
    assert!(presets.values(None).gpu_cluster_culling);
}

#[test]
fn an_unknown_key_fails_listing_the_keys() {
    let e = set(&["gpu_occlusion=true"]).unwrap_err();
    assert!(e.contains("no 'gpu_occlusion'"), "{e}");
    assert!(
        e.contains("gpu_occlusion_culling") && e.contains("ssao"),
        "{e}"
    );
    let e = set(&["ssao.ultra.radius=1"]).unwrap_err();
    assert!(
        e.contains("low, medium") || e.contains("high"),
        "the level's keys: {e}"
    );
}

#[test]
fn a_malformed_pair_or_a_wrong_shape_fails() {
    assert!(set(&["gpu_occlusion_culling"])
        .unwrap_err()
        .contains("key=value"));
    assert!(
        set(&["gpu_occlusion_culling=maybe"]).is_err(),
        "a bool wants a bool"
    );
}

#[test]
fn no_override_changes_nothing() {
    let mut presets = QualityPresets::default();
    let o = set(&[]).unwrap();
    assert!(o.is_empty());
    o.apply(&mut presets).unwrap();
    assert_eq!(presets, QualityPresets::default());
}

#[test]
fn an_option_key_sets_the_project_default_and_a_render_key_its_switch() {
    use rusty_core::settings::graphics_options::GraphicsOption;
    use rusty_render::RenderSwitch;
    let o = set(&["option.ssao=false", "render.shadows=false"]).unwrap();
    let mut settings = rusty_core::project::ProjectSettings::default();
    let quality = settings.quality.clone();
    o.apply_settings(&mut settings).unwrap();
    assert!(!settings.graphics_options.allows(GraphicsOption::Ssao));
    assert!(settings.graphics_options.allows(GraphicsOption::Ssil));
    assert_eq!(settings.quality, quality, "no quality field moved");
    assert!(!o.switches().on(RenderSwitch::Shadows));
    assert!(o.switches().on(RenderSwitch::Lightmaps));
    assert!(set(&["gpu_occlusion_culling=true"])
        .unwrap()
        .switches()
        .all_on());
}

#[test]
fn an_unknown_option_or_switch_fails_listing_every_namespace() {
    for key in [
        "option.ssa=false",
        "render.shadow=false",
        "rendr.shadows=false",
    ] {
        let e = set(&[key]).unwrap_err();
        assert!(e.contains("option.<name>: bloom"), "{key}: {e}");
        assert!(
            e.contains("render.<switch>: shadows, lightmaps"),
            "{key}: {e}"
        );
    }
    let e = set(&["render.shadows=0"]).unwrap_err();
    assert!(e.contains("wants true or false"), "{e}");
}

#[test]
fn render_flushes_picks_a_shape_and_refuses_another_value() {
    use rusty_render::EarlyFlushes;
    for want in EarlyFlushes::ALL {
        let o = set(&[&format!("render.flushes={want}")]).unwrap();
        assert_eq!(o.switches().flushes(), want);
    }
    assert!(!set(&["render.flushes=none"]).unwrap().switches().all_on());
    let e = set(&["render.flushes=false"]).unwrap_err();
    assert!(e.contains("none, occlusion, occlusion_scene"), "{e}");
}
