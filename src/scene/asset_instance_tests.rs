//! Tests for `instantiate_asset` (#182): one entity from a `path::sub_object`
//! reference — placed at the requested transform, carrying an "Asset" mesh that
//! rehydrates from its reference (no inlined GPU buffers), its glTF material (deduped
//! into the library) or the engine default, and a synced collider via the sidecar.

use glam::Vec3;

use super::instantiate_asset;
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

/// A glTF whose mesh names NO material — must be dressed with the engine default.
const NO_MATERIAL_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [ { "mesh": 0 } ],
  "meshes": [ { "name": "Bare", "primitives": [ { "attributes": { "POSITION": 0 } } ] } ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }
  ],
  "bufferViews": [ { "buffer": 0, "byteOffset": 0, "byteLength": 36 } ],
  "buffers": [ { "byteLength": 36,
    "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" } ]
}
"#;

fn write_gltf(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn instantiate_places_textured_entity_and_dedups_material() {
    let path = write_gltf("rusty_182_place.gltf", SHARED_MATERIAL_GLTF);
    let mut scene = Scene::new();

    let pos = Vec3::new(2.0, 0.0, -5.0);
    let a = instantiate_asset(&mut scene, &format!("{path}::PartA"), None, pos).unwrap();
    let b = instantiate_asset(
        &mut scene,
        &format!("{path}::PartB"),
        Some("Tower"),
        Vec3::ZERO,
    )
    .unwrap();

    // Placed at the requested transform; named from the sub-object when None.
    assert_eq!(scene.world.transform(a).unwrap().position, pos);
    assert_eq!(*scene.world.name(a).unwrap(), "PartA");
    assert_eq!(*scene.world.name(b).unwrap(), "Tower");

    // Mesh is a reference-backed "Asset" with real geometry, not inlined.
    let mesh = scene.world.mesh(a).expect("asset mesh attached");
    assert_eq!(mesh.primitive_type, "Asset");
    assert_eq!(
        mesh.asset_ref.as_deref(),
        Some(format!("{path}::PartA").as_str())
    );
    assert!(!mesh.vertices.is_empty());
    drop(mesh);

    // Both sub-objects share ONE library key (dedup).
    let key = format!("{path}::Shared");
    assert_eq!(scene.materials.len(), 1);
    assert_eq!(scene.world.material(a).unwrap().material, key);
    assert_eq!(scene.world.material(b).unwrap().material, key);
}

#[test]
fn instantiated_asset_round_trips_as_a_reference() {
    let path = write_gltf("rusty_182_roundtrip.gltf", SHARED_MATERIAL_GLTF);
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::PartA"), None, Vec3::ZERO).unwrap();

    // Save preserves the reference, never the vertices.
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(json.contains("\"primitive_type\":\"Asset\""));
    assert!(!json.contains("\"vertices\""));

    // Reload re-imports geometry from the reference and keeps the material.
    let mut restored = Scene::new();
    apply_scene_data(&mut restored, to_scene_data(&scene));
    let mesh = restored.world.mesh(id).unwrap().clone();
    assert_eq!(
        mesh.asset_ref.as_deref(),
        Some(format!("{path}::PartA").as_str())
    );
    assert!(!mesh.vertices.is_empty());
    assert!(restored.materials.contains_key(&format!("{path}::Shared")));
}

#[test]
fn material_less_asset_gets_engine_default() {
    let path = write_gltf("rusty_182_default.gltf", NO_MATERIAL_GLTF);
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::Bare"), None, Vec3::ZERO).unwrap();

    let m = scene.world.material(id).expect("default material");
    assert!(
        scene.materials.contains_key(&m.material),
        "reference resolves"
    );
}

#[test]
fn bad_reference_errors_without_spawning() {
    let path = write_gltf("rusty_182_bad.gltf", SHARED_MATERIAL_GLTF);
    let mut scene = Scene::new();
    let before = scene.world.collect_entities().len();

    assert!(instantiate_asset(&mut scene, "no-separator", None, Vec3::ZERO).is_err());
    assert!(instantiate_asset(&mut scene, &format!("{path}::Nope"), None, Vec3::ZERO).is_err());
    assert_eq!(
        scene.world.collect_entities().len(),
        before,
        "no phantom entity"
    );
}
