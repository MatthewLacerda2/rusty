//! src/asset/fixtures.rs — tiny embedded source assets for the import tests.
//!
//! Hand-written minimal files (one named sub-object each) so the importer's tests
//! are self-contained and don't ship binary blobs. `#[cfg(test)]`-only.

/// A two-triangle (quad) `.obj` with a single named object `Quad`.
pub const TRIANGLE_OBJ: &str = "\
o Quad
v -1.0 0.0 -1.0
v 1.0 0.0 -1.0
v 1.0 0.0 1.0
v -1.0 0.0 1.0
vn 0.0 1.0 0.0
vt 0.0 0.0
vt 1.0 0.0
vt 1.0 1.0
vt 0.0 1.0
f 1/1/1 2/2/1 3/3/1
f 1/1/1 3/3/1 4/4/1
";

/// A minimal embedded-buffer glTF 2.0 file: one mesh named `Triangle`, one
/// POSITION-only primitive (a single triangle), one base-color material.
pub const TRIANGLE_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [ { "mesh": 0 } ],
  "meshes": [
    {
      "name": "Triangle",
      "primitives": [
        { "attributes": { "POSITION": 0 }, "material": 0 }
      ]
    }
  ],
  "materials": [
    {
      "name": "Red",
      "pbrMetallicRoughness": { "baseColorFactor": [1.0, 0.0, 0.0, 1.0] }
    }
  ],
  "accessors": [
    {
      "bufferView": 0,
      "componentType": 5126,
      "count": 3,
      "type": "VEC3",
      "min": [0.0, 0.0, 0.0],
      "max": [1.0, 1.0, 0.0]
    }
  ],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 36 }
  ],
  "buffers": [
    {
      "byteLength": 36,
      "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA"
    }
  ]
}
"#;
