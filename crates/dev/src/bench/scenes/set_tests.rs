//! `--set`: a known key changes the preset the run plays, nested keys and plain
//! strings included; an unknown key or a value of the wrong shape fails with
//! what there is; no override leaves the presets as they were.

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
