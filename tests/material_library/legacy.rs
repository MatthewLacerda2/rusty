//! Back-compat: pre-#201 scenes with an inline `texture` still load.

use rusty::scene::{apply_scene_data, Scene};

/// An OLD-format scene (entity with inline `"texture"`, no `materials`) as JSON.
/// `color`/`path` set the migrated values; the rest are minimal valid fields.
fn legacy_scene_json() -> String {
    r#"{
        "entities": [{
            "id": 7, "name": "OldBox", "active": true, "is_static": false,
            "transform": { "position": [0,0,0], "rotation": [0,0,0,1], "scale": [1,1,1] },
            "mesh": null,
            "texture": { "path": "tex/brick.png", "is_dirty": false, "metallic": 0.3,
                "roughness": 0.7, "metallic_map": null, "roughness_map": null,
                "color": [0.2, 0.4, 0.6] },
            "scripts": [], "parent_id": null, "children": []
        }],
        "next_entity_id": 8, "selected_entity_id": null
    }"#
    .to_string()
}

#[test]
fn legacy_inline_texture_migrates_into_library_material() {
    // A pre-#201 inline `texture` must load with no data loss into a library
    // material + a synthesized reference.
    let data: rusty::scene::SceneData =
        serde_json::from_str(&legacy_scene_json()).expect("legacy scene parses");
    let mut scene = Scene::new();
    apply_scene_data(&mut scene, data);

    let e = scene.world.entity_document(7).unwrap();
    let key = &e.material.as_ref().expect("reference attached").material;
    assert_eq!(key, "entity_7_material");
    let mat = scene
        .material_asset_of(7)
        .expect("legacy material migrated");
    assert_eq!(mat.base_color, [0.2, 0.4, 0.6]);
    assert_eq!(mat.base_color_map.as_deref(), Some("tex/brick.png"));
    assert_eq!(mat.metallic, 0.3);
    assert_eq!(mat.roughness, 0.7);
    assert!(
        e.pending_material.is_none(),
        "carrier cleared after migration"
    );
}

#[test]
fn corpus_seed_still_loads() {
    // The full-document corpus seed is OLD-format (inline `texture`, no `materials`);
    // it MUST load via the legacy migration without panicking, proving the
    // back-compat path against a real checked-in file. (The tracked default.scene it
    // was copied from is now built in Rust, #667.)
    for path in ["fuzz/corpus/scene_deserialize/seed_default.scene"] {
        let text = std::fs::read_to_string(path).unwrap();
        let data: rusty::scene::SceneData =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
        apply_scene_data(&mut Scene::new(), data);
    }
}
