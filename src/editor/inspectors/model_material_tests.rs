//! Tests for the model inspector's material-import wiring (#203): `import_material`
//! deduplication + `instantiate` populating `scene.materials` and the entity
//! `MaterialComponent`, plus a `SceneData` round-trip of an imported material.

use super::{import_material, instantiate};
use crate::editor::EditorUi;
use crate::scene::{apply_scene_data, to_scene_data, Scene};

/// A glTF with TWO meshes that both reference material 0 — proves sub-objects sharing
/// one glTF material share ONE library key (dedup). One embedded buffer holds three
/// shared positions (both meshes point at the same accessor).
const SHARED_MATERIAL_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0, 1 ] } ],
  "nodes": [ { "mesh": 0 }, { "mesh": 1 } ],
  "meshes": [
    { "name": "PartA", "primitives": [ { "attributes": { "POSITION": 0 }, "material": 0 } ] },
    { "name": "PartB", "primitives": [ { "attributes": { "POSITION": 0 }, "material": 0 } ] }
  ],
  "materials": [
    { "name": "Shared", "pbrMetallicRoughness": {
        "baseColorFactor": [0.2, 0.6, 0.9, 1.0], "metallicFactor": 0.3, "roughnessFactor": 0.4 } }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }
  ],
  "bufferViews": [ { "buffer": 0, "byteOffset": 0, "byteLength": 36 } ],
  "buffers": [ { "byteLength": 36,
    "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" } ]
}
"#;

fn write_fixture(name: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, SHARED_MATERIAL_GLTF).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn import_material_dedups_shared_gltf_material() {
    let path = write_fixture("rusty_203_dedup.gltf");
    let asset = crate::asset::import_file(std::path::Path::new(&path)).unwrap();
    let mut scene = Scene::new();

    // Both sub-meshes reference material 0 → same deterministic key, one library entry.
    let key_a = import_material(&mut scene, &path, &asset, 0).unwrap();
    let key_b = import_material(&mut scene, &path, &asset, 0).unwrap();
    assert_eq!(key_a, key_b);
    assert_eq!(key_a, format!("{path}::Shared"));
    assert_eq!(scene.materials.len(), 1);
    let mat = &scene.materials[&key_a];
    assert_eq!(mat.base_color, [0.2, 0.6, 0.9]);
    assert_eq!(mat.metallic, 0.3);
    assert_eq!(mat.roughness, 0.4);

    // Out-of-range index yields no key/entry.
    assert!(import_material(&mut scene, &path, &asset, 9).is_none());
}

#[test]
fn instantiate_sets_material_component_and_shares_library_entry() {
    let path = write_fixture("rusty_203_instantiate.gltf");
    let mut scene = Scene::new();
    let mut editor = EditorUi::new();
    instantiate(&mut editor, &mut scene, &path);

    // Two entities, each with the mesh's shared material reference; ONE library entry.
    let key = format!("{path}::Shared");
    assert_eq!(scene.materials.len(), 1);
    assert!(scene.materials.contains_key(&key));
    let refs: Vec<String> = scene
        .world
        .collect_entities()
        .iter()
        .filter_map(|e| e.material.as_ref().map(|m| m.material.clone()))
        .collect();
    assert_eq!(refs.len(), 2, "both sub-objects carry a MaterialComponent");
    assert!(refs.iter().all(|r| *r == key), "both share one library key");
}

#[test]
fn imported_material_round_trips_through_scene_data() {
    let path = write_fixture("rusty_203_roundtrip.gltf");
    let mut scene = Scene::new();
    let mut editor = EditorUi::new();
    instantiate(&mut editor, &mut scene, &path);

    let data = to_scene_data(&scene);
    let mut restored = Scene::new();
    apply_scene_data(&mut restored, data);

    let key = format!("{path}::Shared");
    let mat = restored
        .materials
        .get(&key)
        .expect("material survived save/load");
    assert_eq!(mat.base_color, [0.2, 0.6, 0.9]);
    assert_eq!(mat.metallic, 0.3);
    assert_eq!(mat.roughness, 0.4);
    let count = restored
        .world
        .collect_entities()
        .iter()
        .filter(|e| {
            e.material
                .as_ref()
                .map(|m| m.material == key)
                .unwrap_or(false)
        })
        .count();
    assert_eq!(count, 2, "both material references round-tripped");
}
