//! Tests for the model inspector's material wiring: `import_material` dedup +
//! `instantiate` populating `scene.materials` and the entity `MaterialComponent`
//! (#203), a `SceneData` round-trip, and the engine-default fallback for a
//! material-less glTF (#214).

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

/// A glTF whose two meshes name NO material — the case the engine must dress with its
/// default `Material` (#214). Same shared-position buffer as above, refs dropped.
const NO_MATERIAL_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0, 1 ] } ],
  "nodes": [ { "mesh": 0 }, { "mesh": 1 } ],
  "meshes": [
    { "name": "Bare0", "primitives": [ { "attributes": { "POSITION": 0 } } ] },
    { "name": "Bare1", "primitives": [ { "attributes": { "POSITION": 0 } } ] }
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
    write_gltf(name, SHARED_MATERIAL_GLTF)
}

fn write_gltf(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).unwrap();
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
    instantiate(&mut EditorUi::new(), &mut scene, &path);

    // Two entities, each with the mesh's shared material reference; ONE library entry.
    let key = format!("{path}::Shared");
    assert_eq!(scene.materials.len(), 1);
    assert!(scene.materials.contains_key(&key));
    let entities = scene.world.collect_entities();
    let refs: Vec<_> = entities
        .iter()
        .filter_map(|e| e.material.as_ref())
        .collect();
    assert_eq!(refs.len(), 2, "both sub-objects carry a MaterialComponent");
    assert!(refs.iter().all(|m| m.material == key), "both share one key");
}

#[test]
fn imported_material_round_trips_through_scene_data() {
    let path = write_fixture("rusty_203_roundtrip.gltf");
    let mut scene = Scene::new();
    instantiate(&mut EditorUi::new(), &mut scene, &path);

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
        .filter(|e| e.material.as_ref().is_some_and(|m| m.material == key))
        .count();
    assert_eq!(count, 2, "both material references round-tripped");
}

#[test]
fn instantiate_dresses_material_less_meshes_with_engine_default() {
    let path = write_gltf("rusty_214_default.gltf", NO_MATERIAL_GLTF);
    let mut scene = Scene::new();
    instantiate(&mut EditorUi::new(), &mut scene, &path);

    // Both created mesh entities carry a RESOLVABLE material (key present in the shared
    // library) even though the glTF named none.
    let entities = scene.world.collect_entities();
    let meshed: Vec<_> = entities.iter().filter(|e| e.mesh.is_some()).collect();
    assert_eq!(meshed.len(), 2, "both sub-objects instantiated");
    for e in meshed {
        let m = e.material.as_ref().expect("default material attached");
        assert!(scene.materials.contains_key(&m.material), "ref resolves");
    }
}
