//! What the import hands the bake (#438, #831): a mesh without `TEXCOORD_1` bakes
//! nothing until *Generate Lightmap UVs* is ticked, and the unwrap's pack margin
//! is measured at the bake's default resolution. The import halves are
//! `rusty-asset`'s own tests (`gltf`'s `uv2_tests`, `lightmap_uv`'s).

use rusty_asset::lightmap_uv::REFERENCE_TEXELS_PER_UNIT;
use rusty_asset::{import_file, SubMesh};
use rusty_test_support::models::quad_mode_gltf;

#[test]
fn the_margin_is_measured_at_the_bakes_default_resolution() {
    assert_eq!(
        REFERENCE_TEXELS_PER_UNIT,
        super::BakeSettings::default().texels_per_unit
    );
}

/// Write a JSON + `<stem>.bin` pair into its own temp dir and import its one mesh.
fn import_pair(dir: &str, stem: &str, (json, bin): (String, Vec<u8>)) -> SubMesh {
    let dir = rusty_test_support::temp::dir().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{stem}.bin")), bin).unwrap();
    let path = dir.join(format!("{stem}.gltf"));
    std::fs::write(&path, json).unwrap();
    import_file(&path).unwrap().sub_meshes.remove(0)
}

/// The bake's input for one imported mesh, placed at the origin under a white sky.
fn bake_input(mesh: &SubMesh) -> super::BakeScene {
    use super::{BakeMesh, BakeRun, BakeScene};
    let field =
        |f: fn(&rusty_asset::MeshVertex) -> glam::Vec3| mesh.vertices.iter().map(f).collect();
    BakeScene {
        shadowmask: Default::default(),
        meshes: vec![BakeMesh {
            entity: 1,
            positions: field(|v| v.position.into()),
            normals: field(|_| glam::Vec3::Z),
            lightmap_uvs: mesh.vertices.iter().map(|v| v.lightmap_uv.into()).collect(),
            indices: mesh.indices.clone(),
            runs: vec![BakeRun {
                end: mesh.indices.len() as u32,
                albedo: glam::Vec3::ONE,
                emissive: glam::Vec3::ZERO,
                occludes: true,
            }],
            ..Default::default()
        }],
        lights: Vec::new(),
        sky: glam::Vec3::ONE.into(),
        diffuse: Default::default(),
    }
}

#[test]
fn ticking_generate_lightmap_uvs_makes_a_mesh_without_texcoord_1_bake() {
    use super::{bake, BakeSettings};
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

    let dir = rusty_test_support::temp::dir().join("rusty_uv2_gen_on");
    std::fs::create_dir_all(&dir).unwrap();
    let on = rusty_asset::LightmapUvSettings {
        generate: true,
        ..Default::default()
    };
    rusty_asset::sidecar::set_lightmap_uvs(&dir.join("quad.gltf"), on).unwrap();
    let mesh = import_pair(
        "rusty_uv2_gen_on",
        "quad",
        quad_mode_gltf(4, &[0, 1, 2, 0, 2, 3]),
    );
    assert!(mesh.vertices.iter().any(|v| v.lightmap_uv != [0.0, 0.0]));
    assert_eq!(bake(&bake_input(&mesh), &settings).len(), 1);
}
