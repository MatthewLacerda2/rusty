use serde_json::json;

use super::*;
use crate::core::settings::quality::QualityPreset;

fn temp_path(name: &str) -> std::path::PathBuf {
    let path = crate::test_temp::dir().join(format!(
        "rusty_graphics_options_{}_{name}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn every_option_defaults_to_allowed() {
    let options = GraphicsOptions::default();
    assert!(GraphicsOption::ALL.iter().all(|&o| options.allows(o)));
}

/// Set, store, flush, reopen: the same options come back (#1438).
#[test]
fn options_round_trip_through_storage_on_disk() {
    let path = temp_path("round_trip");
    let mut options = GraphicsOptions::default();
    options.set(GraphicsOption::MotionBlur, false);
    options.set(GraphicsOption::Ssao, false);

    let mut storage = Storage::new();
    storage.open(&path).unwrap();
    options.store(&mut storage);
    storage.flush().unwrap();

    let mut reopened = Storage::new();
    reopened.open(&path).unwrap();
    assert_eq!(
        GraphicsOptions::load(&reopened, GraphicsOptions::default()),
        options
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_missing_namespace_reads_as_the_defaults_it_is_given() {
    let storage = Storage::new();
    assert_eq!(
        GraphicsOptions::load(&storage, GraphicsOptions::default()),
        GraphicsOptions::default()
    );
    let mut project = GraphicsOptions::default();
    project.set(GraphicsOption::Bloom, false);
    assert_eq!(GraphicsOptions::load(&storage, project), project);
}

/// A partial blob overrides only what it names; junk is ignored, not an error.
#[test]
fn a_partial_blob_falls_back_per_option() {
    let mut project = GraphicsOptions::default();
    project.set(GraphicsOption::MotionBlur, false);
    let blob = json!({ "fxaa": false, "warp_drive": false, "bloom": "no" });
    let read = GraphicsOptions::from_json(&blob, project);
    assert!(
        !read.allows(GraphicsOption::MotionBlur),
        "project default kept"
    );
    assert!(!read.allows(GraphicsOption::Fxaa), "player choice read");
    assert!(
        read.allows(GraphicsOption::Bloom),
        "a non-bool changes nothing"
    );
    assert_eq!(GraphicsOptions::from_json(&json!(3), project), project);
}

#[test]
fn serde_spells_the_options_by_name() {
    let mut options = GraphicsOptions::default();
    options.set(GraphicsOption::Ssr, false);
    let text = serde_json::to_string(&options).unwrap();
    assert!(text.contains("\"ssr\":false"), "{text}");
    assert_eq!(
        serde_json::from_str::<GraphicsOptions>(&text).unwrap(),
        options
    );
}

#[test]
fn the_tier_allows_what_its_passes_run() {
    use GraphicsOption::*;
    assert!(!QualityPreset::Low.allows(MotionBlur));
    assert!(!QualityPreset::Low.allows(Ssao));
    assert!(!QualityPreset::Medium.allows(Ssr));
    assert!(QualityPreset::High.allows(Ssr));
    for tier in QualityPreset::ALL {
        assert!(tier.allows(Fxaa) && tier.allows(Bloom), "{tier}");
    }
}
