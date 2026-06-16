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

/// A minimal skinned glTF 2.0 file (issue #79): one mesh `Skinned` with a single
/// triangle whose vertices carry `JOINTS_0`/`WEIGHTS_0`, and a 2-joint `skin`. The
/// joints form a parent→child chain (`joint0` at +X, `joint1` a child at +Y) with
/// inverse-bind matrices that invert each joint's bind-pose global — so the bind
/// palette is the identity, which proves the hierarchy walk and inverse-bind read.
///
/// The geometry/skin live in a sibling `model.bin` ([`skinned_buffer`]); the test
/// writes both before importing. `byteLength` matches that buffer (224 bytes).
pub const SKINNED_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0, 1 ] } ],
  "nodes": [
    { "name": "MeshNode", "mesh": 0, "skin": 0 },
    { "name": "Joint0", "translation": [1.0, 0.0, 0.0], "children": [ 2 ] },
    { "name": "Joint1", "translation": [0.0, 2.0, 0.0] }
  ],
  "meshes": [
    {
      "name": "Skinned",
      "primitives": [
        { "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 } }
      ]
    }
  ],
  "skins": [
    { "joints": [ 1, 2 ], "inverseBindMatrices": 3 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] },
    { "bufferView": 1, "componentType": 5121, "count": 3, "type": "VEC4" },
    { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
    { "bufferView": 3, "componentType": 5126, "count": 2, "type": "MAT4" }
  ],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0,  "byteLength": 36 },
    { "buffer": 0, "byteOffset": 36, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 48, "byteLength": 48 },
    { "buffer": 0, "byteOffset": 96, "byteLength": 128 }
  ],
  "buffers": [ { "byteLength": 224, "uri": "model.bin" } ]
}
"#;

/// The binary buffer for [`SKINNED_GLTF`], built little-endian: 3 positions, then
/// `JOINTS_0` (u8 vec4), `WEIGHTS_0` (f32 vec4), then 2 column-major inverse-bind
/// matrices (the inverses of `translate(1,0,0)` and `translate(1,2,0)`).
pub fn skinned_buffer() -> Vec<u8> {
    fn put_f32(buf: &mut Vec<u8>, vals: &[f32]) {
        for v in vals {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }
    let mut buf = Vec::new();
    put_f32(&mut buf, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    buf.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0]); // joints (u8)
    put_f32(
        &mut buf,
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0],
    );
    #[rustfmt::skip]
    put_f32(&mut buf, &[
        1.0, 0.0, 0.0, 0.0,  0.0, 1.0, 0.0, 0.0,  0.0, 0.0, 1.0, 0.0,  -1.0,  0.0, 0.0, 1.0,
        1.0, 0.0, 0.0, 0.0,  0.0, 1.0, 0.0, 0.0,  0.0, 0.0, 1.0, 0.0,  -1.0, -2.0, 0.0, 1.0,
    ]);
    buf
}
