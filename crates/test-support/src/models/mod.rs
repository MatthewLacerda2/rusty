pub mod anim;
mod fbx;
mod glb;
mod quad;

pub use fbx::{embedded_fbx, png, Embedded};
pub use glb::glb;
pub use quad::quad_file;

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

/// The image is resolved, never read, so a test writes the `.gltf` alone.
pub const TEXTURED_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [ { "mesh": 0 } ],
  "meshes": [
    {
      "name": "Brick",
      "primitives": [
        { "attributes": { "POSITION": 0 }, "material": 0 }
      ]
    }
  ],
  "materials": [
    {
      "name": "Bricks",
      "pbrMetallicRoughness": {
        "baseColorFactor": [0.8, 0.4, 0.2, 1.0],
        "metallicFactor": 0.25,
        "roughnessFactor": 0.75,
        "baseColorTexture": { "index": 0 }
      },
      "emissiveFactor": [0.1, 0.2, 0.3]
    }
  ],
  "textures": [ { "source": 0 } ],
  "images": [ { "uri": "tex/brick%20wall.png" } ],
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

/// Its `model.bin` is [`skinned_buffer`]: the views and the 224 bytes follow its
/// layout.
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

/// Its normals keep the indices as written: the import flat-shades a normal-less
/// primitive.
pub fn quad_mode_gltf(mode: u32, indices: &[u16]) -> (String, Vec<u8>) {
    let mut bin = Vec::new();
    let corners: [f32; 12] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0];
    for c in corners.iter().chain(&[0.0, 0.0, 1.0].repeat(4)) {
        bin.extend_from_slice(&c.to_le_bytes());
    }
    for i in indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    let (indices_key, index_accessor, index_view) = if indices.is_empty() {
        (String::new(), String::new(), String::new())
    } else {
        (
            r#", "indices": 2"#.to_string(),
            format!(
                r#",
    {{ "bufferView": 2, "componentType": 5123, "count": {}, "type": "SCALAR" }}"#,
                indices.len()
            ),
            format!(
                r#",
    {{ "buffer": 0, "byteOffset": 96, "byteLength": {} }}"#,
                indices.len() * 2
            ),
        )
    };
    let json = format!(
        r#"{{
  "asset": {{ "version": "2.0" }},
  "scene": 0,
  "scenes": [ {{ "nodes": [ 0 ] }} ],
  "nodes": [ {{ "mesh": 0 }} ],
  "meshes": [
    {{ "name": "Quad",
      "primitives": [ {{ "attributes": {{ "POSITION": 0, "NORMAL": 1 }}, "mode": {mode}{indices_key} }} ] }}
  ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }},
    {{ "bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3" }}{index_accessor}
  ],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": 48 }},
    {{ "buffer": 0, "byteOffset": 48, "byteLength": 48 }}{index_view}
  ],
  "buffers": [ {{ "byteLength": {}, "uri": "quad.bin" }} ]
}}
"#,
        bin.len()
    );
    (json, bin)
}

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
