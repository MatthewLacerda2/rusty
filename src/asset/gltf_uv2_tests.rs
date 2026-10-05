//! src/asset/gltf_uv2_tests.rs — the lightmap UV import (#438): glTF `TEXCOORD_1`
//! (Blender's second UV map) lands in `MeshVertex::lightmap_uv`, and a mesh without
//! one imports all-zero lightmap UVs, the bake's "no lightmap" signal.

use super::fixtures::quad_mode_gltf;
use super::import_file;
use super::mesh_data::SubMesh;

const UV0: [[f32; 2]; 3] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
const UV1: [[f32; 2]; 3] = [[0.1, 0.2], [0.9, 0.2], [0.1, 0.8]];

/// One triangle carrying `TEXCOORD_0` and `TEXCOORD_1`, as a JSON + buffer pair.
fn two_uv_triangle() -> (String, Vec<u8>) {
    let mut bin = Vec::new();
    let positions = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let uvs = UV0.iter().chain(UV1.iter()).flatten();
    for f in positions.iter().chain(uvs) {
        bin.extend_from_slice(&f.to_le_bytes());
    }
    let json = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [ { "mesh": 0 } ],
  "meshes": [ { "name": "Tri", "primitives": [ { "attributes":
    { "POSITION": 0, "TEXCOORD_0": 1, "TEXCOORD_1": 2 } } ] } ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC2" },
    { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" }
  ],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
    { "buffer": 0, "byteOffset": 36, "byteLength": 24 },
    { "buffer": 0, "byteOffset": 60, "byteLength": 24 }
  ],
  "buffers": [ { "byteLength": 84, "uri": "tri.bin" } ]
}"#;
    (json.to_string(), bin)
}

/// Write a JSON + `<stem>.bin` pair into its own temp dir and import its one mesh.
fn import_pair(dir: &str, stem: &str, (json, bin): (String, Vec<u8>)) -> SubMesh {
    let dir = std::env::temp_dir().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{stem}.bin")), bin).unwrap();
    let path = dir.join(format!("{stem}.gltf"));
    std::fs::write(&path, json).unwrap();
    import_file(&path).unwrap().sub_meshes.remove(0)
}

#[test]
fn texcoord_1_imports_as_the_lightmap_uv() {
    let mesh = import_pair("rusty_uv2_two", "tri", two_uv_triangle());
    let lightmap: Vec<[f32; 2]> = mesh.vertices.iter().map(|v| v.lightmap_uv).collect();
    let texture: Vec<[f32; 2]> = mesh.vertices.iter().map(|v| v.tex_coords).collect();
    assert_eq!(lightmap, UV1.to_vec(), "TEXCOORD_1 is the lightmap UV");
    assert_eq!(texture, UV0.to_vec(), "TEXCOORD_0 stays the texture UV");
}

#[test]
fn a_mesh_without_texcoord_1_has_zero_lightmap_uvs() {
    let mesh = import_pair("rusty_uv2_none", "quad", quad_mode_gltf(4, &[0, 1, 2]));
    assert!(mesh.vertices.iter().all(|v| v.lightmap_uv == [0.0, 0.0]));
}

/// The bake's input for one imported mesh, placed at the origin under a white sky.
fn bake_input(mesh: &SubMesh) -> crate::scene::lighting::lightmap::BakeScene {
    use crate::scene::lighting::lightmap::{BakeMesh, BakeScene};
    let field = |f: fn(&super::MeshVertex) -> glam::Vec3| mesh.vertices.iter().map(f).collect();
    BakeScene {
        meshes: vec![BakeMesh {
            entity: 1,
            positions: field(|v| v.position.into()),
            normals: field(|_| glam::Vec3::Z),
            lightmap_uvs: mesh.vertices.iter().map(|v| v.lightmap_uv.into()).collect(),
            indices: mesh.indices.clone(),
            albedo: glam::Vec3::ONE,
            emissive: glam::Vec3::ZERO,
        }],
        lights: Vec::new(),
        sky: glam::Vec3::ONE,
    }
}

#[test]
fn ticking_generate_lightmap_uvs_makes_a_mesh_without_texcoord_1_bake() {
    use crate::scene::lighting::lightmap::{bake, BakeSettings};
    let settings = BakeSettings {
        samples: 4,
        ..Default::default()
    };
    let plain = import_pair(
        "rusty_uv2_gen_off",
        "quad",
        quad_mode_gltf(4, &[0, 1, 2, 0, 2, 3]),
    );
    assert!(bake(&bake_input(&plain), &settings).is_empty());

    let dir = std::env::temp_dir().join("rusty_uv2_gen_on");
    std::fs::create_dir_all(&dir).unwrap();
    let on = super::LightmapUvSettings {
        generate: true,
        ..Default::default()
    };
    super::sidecar::set_lightmap_uvs(&dir.join("quad.gltf"), on).unwrap();
    let mesh = import_pair(
        "rusty_uv2_gen_on",
        "quad",
        quad_mode_gltf(4, &[0, 1, 2, 0, 2, 3]),
    );
    assert!(mesh.vertices.iter().any(|v| v.lightmap_uv != [0.0, 0.0]));
    assert_eq!(bake(&bake_input(&mesh), &settings).len(), 1);
}
