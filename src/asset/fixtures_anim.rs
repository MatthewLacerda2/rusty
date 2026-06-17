//! src/asset/fixtures_anim.rs — embedded animated-skin fixture for the clip-import
//! tests (#80). Kept apart from `fixtures` to stay under the test-file size cap.
//!
//! One mesh `Animated` with a 2-joint skin and one animation `Wave` that translates
//! `Joint0` from the origin to +X=2 over [0, 1] s (two linear keys). Hand-written so
//! the importer's clip tests need no binary blobs; `#[cfg(test)]`-only.

/// A minimal animated, skinned glTF 2.0 file with one clip `Wave`. The geometry/skin
/// mirror the `SKINNED_GLTF` fixture; the extra accessors (4, 5) carry the
/// animation's input times and output translations, read from a sibling
/// `anim.bin` ([`animated_buffer`], 256 bytes).
pub const ANIMATED_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0, 1 ] } ],
  "nodes": [
    { "name": "MeshNode", "mesh": 0, "skin": 0 },
    { "name": "Joint0", "translation": [0.0, 0.0, 0.0], "children": [ 2 ] },
    { "name": "Joint1", "translation": [0.0, 1.0, 0.0] }
  ],
  "meshes": [
    {
      "name": "Animated",
      "primitives": [
        { "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 } }
      ]
    }
  ],
  "skins": [
    { "joints": [ 1, 2 ], "inverseBindMatrices": 3 }
  ],
  "animations": [
    {
      "name": "Wave",
      "samplers": [
        { "input": 4, "output": 5, "interpolation": "LINEAR" }
      ],
      "channels": [
        { "sampler": 0, "target": { "node": 1, "path": "translation" } }
      ]
    }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] },
    { "bufferView": 1, "componentType": 5121, "count": 3, "type": "VEC4" },
    { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
    { "bufferView": 3, "componentType": 5126, "count": 2, "type": "MAT4" },
    { "bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR",
      "min": [0.0], "max": [1.0] },
    { "bufferView": 5, "componentType": 5126, "count": 2, "type": "VEC3" }
  ],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0,   "byteLength": 36 },
    { "buffer": 0, "byteOffset": 36,  "byteLength": 12 },
    { "buffer": 0, "byteOffset": 48,  "byteLength": 48 },
    { "buffer": 0, "byteOffset": 96,  "byteLength": 128 },
    { "buffer": 0, "byteOffset": 224, "byteLength": 8 },
    { "buffer": 0, "byteOffset": 232, "byteLength": 24 }
  ],
  "buffers": [ { "byteLength": 256, "uri": "anim.bin" } ]
}
"#;

/// The binary buffer for [`ANIMATED_GLTF`], little-endian: 3 positions, `JOINTS_0`
/// (u8 vec4), `WEIGHTS_0` (f32 vec4), 2 identity-ish inverse-bind matrices, then the
/// animation's 2 input times `[0, 1]` and 2 output translations `[(0,0,0),(2,0,0)]`.
pub fn animated_buffer() -> Vec<u8> {
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
        1.0, 0.0, 0.0, 0.0,  0.0, 1.0, 0.0, 0.0,  0.0, 0.0, 1.0, 0.0,  0.0,  0.0, 0.0, 1.0,
        1.0, 0.0, 0.0, 0.0,  0.0, 1.0, 0.0, 0.0,  0.0, 0.0, 1.0, 0.0,  0.0, -1.0, 0.0, 1.0,
    ]);
    put_f32(&mut buf, &[0.0, 1.0]); // input times
    put_f32(&mut buf, &[0.0, 0.0, 0.0, 2.0, 0.0, 0.0]); // output translations
    buf
}
