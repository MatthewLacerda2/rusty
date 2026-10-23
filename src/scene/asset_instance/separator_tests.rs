//! A sub-object whose own name holds `::` (#1295): the reference splits on the
//! first separator, so `file.gltf::Body::Mesh` instantiates, saves, reloads and
//! keeps its mesh.

use glam::Vec3;

use super::instantiate_asset;
use crate::scene::{apply_scene_data, to_scene_data, Scene};

/// One mesh named `Body::Mesh`, the way a glTF exporter can name it.
const COLON_NAMED_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [ { "mesh": 0 } ],
  "meshes": [ { "name": "Body::Mesh", "primitives": [ { "attributes": { "POSITION": 0 } } ] } ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }
  ],
  "bufferViews": [ { "buffer": 0, "byteOffset": 0, "byteLength": 36 } ],
  "buffers": [ { "byteLength": 36,
    "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" } ]
}
"#;

#[test]
fn colon_named_sub_object_survives_save_and_reload() {
    let path = crate::test_temp::dir().join("rusty_1295_colons.gltf");
    std::fs::write(&path, COLON_NAMED_GLTF).unwrap();
    let reference = format!("{}::Body::Mesh", path.to_string_lossy());
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    assert_eq!(*scene.world.name(id).unwrap(), "Body::Mesh");

    let mut restored = Scene::new();
    apply_scene_data(&mut restored, to_scene_data(&scene));
    let mesh = restored.world.mesh(id).expect("mesh kept on reload");
    assert_eq!(mesh.asset_ref.as_deref(), Some(reference.as_str()));
    assert!(!mesh.vertices.is_empty(), "geometry re-imported");
}
