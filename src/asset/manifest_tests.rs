//! src/asset/manifest_tests.rs — manifest walk + round-trip tests over the
//! embedded fixtures written into a temp asset root.

use super::fixtures::{TRIANGLE_GLTF, TRIANGLE_OBJ};
use super::manifest::build_manifest;
use super::{import_sub_mesh, AssetRef};

/// A fresh, unique temp dir to act as an isolated asset root per test.
fn tmp_root(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("rusty_manifest_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn walks_nested_dirs_and_lists_sub_objects() {
    let root = tmp_root("walk");
    let sub = root.join("nested");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(root.join("quad.obj"), TRIANGLE_OBJ).unwrap();
    std::fs::write(sub.join("tri.gltf"), TRIANGLE_GLTF).unwrap();
    // A non-model file must be ignored.
    std::fs::write(root.join("notes.txt"), "ignore me").unwrap();

    let manifest = build_manifest(&root);

    assert_eq!(manifest.assets.len(), 2);
    assert_eq!(manifest.sub_object_count(), 2);
    // Paths are sorted; the nested glTF sorts before the top-level obj.
    assert!(manifest.assets[0].path.ends_with("nested/tri.gltf"));
    assert!(manifest.assets[1].path.ends_with("quad.obj"));
}

#[test]
fn references_round_trip_through_import_sub_mesh() {
    let root = tmp_root("roundtrip");
    std::fs::write(root.join("tri.gltf"), TRIANGLE_GLTF).unwrap();

    let manifest = build_manifest(&root);
    let entry = &manifest.assets[0];
    let sub = &entry.sub_objects[0];

    // The reference parses back into the same path + sub-object...
    let parsed = AssetRef::parse(&sub.reference).unwrap();
    assert_eq!(parsed.path, entry.path);
    assert_eq!(parsed.sub_object, sub.id);
    // ...and re-imports the exact sub-mesh the manifest named.
    let remesh = import_sub_mesh(&sub.reference).unwrap();
    assert_eq!(remesh.id, sub.id);
    assert_eq!(remesh.bounds(), sub.bounds);
}

#[test]
fn records_bounds_and_material_count() {
    let root = tmp_root("bounds");
    std::fs::write(root.join("quad.obj"), TRIANGLE_OBJ).unwrap();

    let manifest = build_manifest(&root);
    let sub = &manifest.assets[0].sub_objects[0];

    let (min, max) = sub.bounds.unwrap();
    assert_eq!(min, glam::Vec3::new(-1.0, 0.0, -1.0));
    assert_eq!(max, glam::Vec3::new(1.0, 0.0, 1.0));
    assert_eq!(sub.size(), glam::Vec3::new(2.0, 0.0, 2.0));
    // The glTF fixture carries one material; the obj quad carries none.
    assert_eq!(sub.material_count, 0);
}

#[test]
fn gltf_material_count_is_one() {
    let root = tmp_root("mat");
    std::fs::write(root.join("tri.gltf"), TRIANGLE_GLTF).unwrap();

    let manifest = build_manifest(&root);
    let entry = &manifest.assets[0];
    assert_eq!(entry.material_count, 1);
    assert_eq!(entry.sub_objects[0].material_count, 1);
}

#[test]
fn missing_root_yields_empty_manifest() {
    let root = std::env::temp_dir().join("rusty_manifest_does_not_exist_xyz");
    let _ = std::fs::remove_dir_all(&root);
    let manifest = build_manifest(&root);
    assert!(manifest.assets.is_empty());
    assert_eq!(manifest.sub_object_count(), 0);
}
