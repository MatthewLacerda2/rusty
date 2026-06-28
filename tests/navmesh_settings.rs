//! Per-scene navmesh bake settings (#276).
//!
//! Locks in (a) the fresh-scene defaults equal the historical bake constants, (b) a
//! pre-#276 save file that omits the `nav_settings` block rehydrates with those defaults
//! (back-compat), and (c) authored settings survive a save→load round-trip.

use rusty::navigation::NavMeshSettings;
use rusty::scene::Scene;

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn fresh_scene_uses_default_nav_settings() {
    let scene = Scene::new();
    assert_eq!(scene.nav_settings, NavMeshSettings::default());
    // The defaults equal the historical hardcoded bake constants.
    assert_eq!(scene.nav_settings.max_step, 0.5);
    assert_eq!(scene.nav_settings.max_slope, 1.0);
    assert_eq!(scene.nav_settings.grid_spacing, 1.0);
    assert_eq!(scene.nav_settings.agent_radius, 0.5);
    assert_eq!(scene.nav_settings.agent_height, 2.0);
}

#[test]
fn legacy_scene_without_nav_settings_loads_with_defaults() {
    // A pre-#276 document that omits the nav_settings block entirely. serde fills it
    // from the engine's defaults, matching a freshly created scene (back-compat).
    let legacy = r#"{
        "entities": [],
        "next_entity_id": 1,
        "selected_entity_id": null
    }"#;
    let path = tmp("rusty_legacy_nav_settings.scene");
    std::fs::write(&path, legacy).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.nav_settings, NavMeshSettings::default());
}

#[test]
fn nav_settings_survive_round_trip() {
    let mut scene = Scene::new();
    scene.nav_settings = NavMeshSettings {
        agent_radius: 0.8,
        agent_height: 1.8,
        max_slope: 0.6,
        max_step: 0.3,
        grid_spacing: 1.5,
    };
    let path = tmp("rusty_nav_settings_roundtrip.scene");
    scene.save_to_file(&path).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.nav_settings.agent_radius, 0.8);
    assert_eq!(loaded.nav_settings.agent_height, 1.8);
    assert_eq!(loaded.nav_settings.max_slope, 0.6);
    assert_eq!(loaded.nav_settings.max_step, 0.3);
    assert_eq!(loaded.nav_settings.grid_spacing, 1.5);
}

#[test]
fn partial_nav_settings_block_fills_missing_fields() {
    // Only some knobs authored: the per-field serde defaults fill the rest, so a hand-
    // edited or partially-migrated scene never loses the other defaults.
    let partial = r#"{
        "entities": [],
        "next_entity_id": 1,
        "selected_entity_id": null,
        "nav_settings": { "max_step": 0.25 }
    }"#;
    let path = tmp("rusty_partial_nav_settings.scene");
    std::fs::write(&path, partial).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.nav_settings.max_step, 0.25, "authored value kept");
    assert_eq!(
        loaded.nav_settings.max_slope, 1.0,
        "missing field defaulted"
    );
    assert_eq!(
        loaded.nav_settings.grid_spacing, 1.0,
        "missing field defaulted"
    );
    assert_eq!(
        loaded.nav_settings.agent_radius, 0.5,
        "missing field defaulted"
    );
    assert_eq!(
        loaded.nav_settings.agent_height, 2.0,
        "missing field defaulted"
    );
}
