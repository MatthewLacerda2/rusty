//! The FBX fixture through the whole import (#1024): `.fbx` → cached `.glb` →
//! the same `ImportedAsset` a glTF gives, units and axes converted.

use crate::asset::{gltf, import_file, ImportedAsset};
use glam::Vec3;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/skinned_cube.fbx")
}

/// The fixture converted fresh into this test's own folder (the shared cache would
/// keep a conversion made by an older build of the converter), then imported.
fn import() -> ImportedAsset {
    let out = crate::test_temp::dir().join("skinned_cube.fbx.glb");
    super::convert(&fixture(), &out).expect("the fixture converts");
    gltf::import::import(&out).expect("its glTF imports")
}

#[test]
fn an_fbx_imports_through_its_cached_gltf() {
    let asset = import_file(&fixture()).expect("the fixture imports");
    assert_eq!(asset.sub_mesh_ids(), vec!["Cube".to_string()]);
}

#[test]
fn the_cube_is_one_sub_mesh_of_twelve_triangles() {
    let asset = import();
    assert_eq!(asset.sub_mesh_ids(), vec!["Cube".to_string()]);
    let cube = &asset.sub_meshes[0];
    assert_eq!(cube.indices.len(), 36);
    assert_eq!(cube.node_name.as_deref(), Some("Cube"));
}

#[test]
fn centimetres_become_metres() {
    let asset = import();
    let cube = &asset.sub_meshes[0];
    for v in &cube.vertices {
        for c in v.position {
            assert!((c.abs() - 0.5).abs() < 1e-4, "corner {:?}", v.position);
        }
    }
}

#[test]
fn every_face_points_away_from_the_centre() {
    let asset = import();
    let cube = &asset.sub_meshes[0];
    for tri in cube.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|i| Vec3::from(cube.vertices[tri[i] as usize].position));
        let centre = (a + b + c) / 3.0;
        assert!(
            (b - a).cross(c - a).dot(centre) > 0.0,
            "inward triangle {tri:?}"
        );
    }
}

#[test]
fn the_lambert_material_keeps_its_colour() {
    let asset = import();
    assert_eq!(asset.materials.len(), 1);
    let red = &asset.materials[0];
    assert_eq!(red.name, "Red");
    let [r, g, b, a] = red.base_color;
    assert!((r - 0.8).abs() < 1e-4 && (g - 0.1).abs() < 1e-4 && (b - 0.1).abs() < 1e-4);
    assert_eq!(a, 1.0);
    assert_eq!(asset.sub_meshes[0].material, Some(0));
}

#[test]
fn the_bone_is_a_joint_one_metre_up_y_weighting_every_vertex() {
    let asset = import();
    let cube = &asset.sub_meshes[0];
    let skin = cube.skin.as_ref().expect("the cube is skinned");
    assert_eq!(skin.names, vec!["Bone".to_string()]);
    let up = skin.bind_global[0].w_axis.truncate();
    assert!((up - Vec3::Y).length() < 1e-4, "bone at {up}");
    // The inverse bind undoes the bone's bind pose: the palette is identity at rest.
    let palette = skin.bind_global[0] * skin.inverse_bind[0];
    assert!(palette.abs_diff_eq(glam::Mat4::IDENTITY, 1e-4), "{palette}");
    for v in &cube.vertices {
        assert_eq!(
            (v.joint_indices, v.joint_weights),
            ([0; 4], [1.0, 0.0, 0.0, 0.0])
        );
    }
}

#[test]
fn converting_twice_writes_the_same_bytes() {
    let dir = crate::test_temp::dir().join("fbx_twice");
    std::fs::create_dir_all(&dir).unwrap();
    let [a, b] = ["a.glb", "b.glb"].map(|n| dir.join(n));
    super::convert(&fixture(), &a).unwrap();
    super::convert(&fixture(), &b).unwrap();
    assert_eq!(std::fs::read(a).unwrap(), std::fs::read(b).unwrap());
}
