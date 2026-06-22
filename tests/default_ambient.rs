//! Default hemisphere-ambient defaults (#256).
//!
//! The out-of-the-box ambient is brightened/coloured (blue-ish sky, darker ground,
//! higher intensity) so differently-oriented faces read without baked probes. These
//! lock in (a) the fresh-scene defaults and (b) that a pre-#256 save file omitting
//! the ambient fields rehydrates with the SAME defaults — i.e. one source of truth.

use rusty::scene::{Scene, DEFAULT_AMBIENT_COLOR, DEFAULT_AMBIENT_INTENSITY};

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn fresh_scene_uses_brightened_ambient_defaults() {
    let scene = Scene::new();
    assert_eq!(scene.ambient_color, DEFAULT_AMBIENT_COLOR);
    assert_eq!(scene.ambient_intensity, DEFAULT_AMBIENT_INTENSITY);
    // The brightened default must be a visible, blue-leaning sky tint — not the old
    // near-black (0.03,0.03,0.045)*0.24 — so faces differ without a probe/light.
    assert!(
        scene.ambient_color.z > scene.ambient_color.x,
        "sky is blue-ish"
    );
    assert!(
        scene.ambient_color.min_element() > 0.1 && scene.ambient_intensity > 0.4,
        "ambient is bright enough to read surfaces"
    );
}

#[test]
fn legacy_scene_without_ambient_loads_with_defaults() {
    // A pre-#256 document that omits the ambient fields entirely. serde fills them
    // from the engine's single source of truth, matching a freshly created scene.
    let legacy = r#"{
        "entities": [],
        "next_entity_id": 1,
        "selected_entity_id": null
    }"#;
    let path = tmp("rusty_legacy_ambient.scene");
    std::fs::write(&path, legacy).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.ambient_color, DEFAULT_AMBIENT_COLOR);
    assert_eq!(loaded.ambient_intensity, DEFAULT_AMBIENT_INTENSITY);
}

#[test]
fn ambient_survives_round_trip() {
    let mut scene = Scene::new();
    scene.ambient_color = glam::Vec3::new(0.1, 0.2, 0.3);
    scene.ambient_intensity = 0.7;
    let path = tmp("rusty_ambient_roundtrip.scene");
    scene.save_to_file(&path).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.ambient_color, glam::Vec3::new(0.1, 0.2, 0.3));
    assert_eq!(loaded.ambient_intensity, 0.7);
}
