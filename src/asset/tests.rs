//! src/asset/tests.rs — importer unit tests over tiny embedded fixtures.

use super::fixtures::{SKINNED_GLTF, TRIANGLE_GLTF, TRIANGLE_OBJ};
use super::*;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("rusty_asset_{name}"));
    p
}

#[test]
fn obj_imports_named_sub_mesh() {
    let path = tmp("quad.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let asset = import_file(&path).unwrap();

    assert_eq!(asset.sub_mesh_ids(), vec!["Quad".to_string()]);
    let quad = asset.sub_mesh("Quad").unwrap();
    assert_eq!(quad.vertices.len(), 4);
    assert_eq!(quad.indices.len(), 6);
    // Normal came through from the `.obj`.
    assert_eq!(quad.vertices[0].normal, [0.0, 1.0, 0.0]);
    let (min, max) = quad.bounds().unwrap();
    assert_eq!(min, glam::Vec3::new(-1.0, 0.0, -1.0));
    assert_eq!(max, glam::Vec3::new(1.0, 0.0, 1.0));
}

#[test]
fn gltf_imports_mesh_and_material() {
    let path = tmp("tri.gltf");
    std::fs::write(&path, TRIANGLE_GLTF).unwrap();
    let asset = import_file(&path).unwrap();

    assert_eq!(asset.sub_mesh_ids(), vec!["Triangle".to_string()]);
    let tri = asset.sub_mesh("Triangle").unwrap();
    assert_eq!(tri.vertices.len(), 3);
    assert_eq!(tri.indices.len(), 3);
    assert_eq!(tri.vertices[1].position, [1.0, 0.0, 0.0]);
    // The base-color material round-tripped.
    assert_eq!(tri.material, Some(0));
    assert_eq!(asset.materials[0].name, "Red");
    assert_eq!(asset.materials[0].base_color, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn gltf_imports_skin_and_bind_pose_palette() {
    // The skinned glTF references a sibling `model.bin`; write both, then import.
    let dir = std::env::temp_dir().join("rusty_skin_fixture");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.bin"), super::fixtures::skinned_buffer()).unwrap();
    let gltf_path = dir.join("model.gltf");
    std::fs::write(&gltf_path, SKINNED_GLTF).unwrap();

    let asset = import_file(&gltf_path).unwrap();
    let sub = asset.sub_mesh("Skinned").unwrap();

    // Per-vertex skin bindings flowed through into the mesh vertices.
    assert_eq!(sub.vertices.len(), 3);
    assert_eq!(sub.vertices[0].joint_indices, [0, 0, 0, 0]);
    assert_eq!(sub.vertices[1].joint_indices, [1, 0, 0, 0]);
    assert_eq!(sub.vertices[2].joint_indices, [0, 1, 0, 0]);
    assert_eq!(sub.vertices[2].joint_weights, [0.5, 0.5, 0.0, 0.0]);

    // The skeleton parsed: two joints, one inverse-bind matrix each.
    let skin = sub.skin.as_ref().expect("skinned sub-mesh has a skeleton");
    assert_eq!(skin.inverse_bind.len(), 2);
    assert_eq!(skin.bind_global.len(), 2);

    // The bind-pose palette is the identity by construction — this validates both
    // the hierarchy walk (joint1 inherits joint0's transform) and the inverse-bind
    // read, since each joint's global is exactly cancelled by its inverse-bind.
    let palette = skin.bind_palette();
    assert_eq!(palette.len(), 2);
    for m in palette {
        assert!(
            m.abs_diff_eq(glam::Mat4::IDENTITY, 1e-5),
            "bind-pose joint matrix should be identity, got {m:?}"
        );
    }
}

#[test]
fn obj_has_no_skin() {
    let path = tmp("noskin.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let sub = import_file(&path).unwrap().sub_meshes.remove(0);
    assert!(sub.skin.is_none());
    // Unskinned vertices keep the identity binding (bone 0, full weight).
    assert_eq!(sub.vertices[0].joint_indices, [0, 0, 0, 0]);
    assert_eq!(sub.vertices[0].joint_weights, [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn import_sub_mesh_by_reference() {
    let path = tmp("ref_quad.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let reference = format!("{}::Quad", path.to_string_lossy());
    let sub = import_sub_mesh(&reference).unwrap();
    assert_eq!(sub.id, "Quad");
    assert_eq!(sub.vertices.len(), 4);
}

#[test]
fn asset_ref_round_trips() {
    let r = AssetRef {
        path: "project/models/crates.glb".to_string(),
        sub_object: "Barrel".to_string(),
    };
    let s = r.to_string_ref();
    assert_eq!(s, "project/models/crates.glb::Barrel");
    assert_eq!(AssetRef::parse(&s).unwrap(), r);
}

#[test]
fn bad_reference_is_rejected() {
    assert!(AssetRef::parse("no_separator_here").is_err());
    assert!(AssetRef::parse("::onlysub").is_err());
    assert!(AssetRef::parse("onlypath::").is_err());
}

#[test]
fn missing_sub_object_errors() {
    let path = tmp("missing.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let reference = format!("{}::Nope", path.to_string_lossy());
    assert!(import_sub_mesh(&reference).is_err());
}

#[test]
fn unsupported_format_errors() {
    assert!(matches!(
        import_file(std::path::Path::new("foo.blend")),
        Err(ImportError::UnsupportedFormat(_))
    ));
    assert!(is_model_path("a/b/c.GLB"));
    assert!(!is_model_path("a/b/c.png"));
}

#[test]
fn sidecar_round_trips_and_caches_sub_objects() {
    let path = tmp("sidecar_quad.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let asset = import_and_sync_sidecar(&path).unwrap();
    assert_eq!(asset.sub_mesh_ids(), vec!["Quad".to_string()]);

    let meta = sidecar::meta_path(&path);
    assert!(meta.exists());
    let loaded = sidecar::load(&path).unwrap();
    assert_eq!(loaded.sub_objects, vec!["Quad".to_string()]);
    assert_eq!(loaded.scale, 1.0);
    assert!(loaded.import_normals);
    // A fresh sidecar requests no mesh colliders (#77).
    assert!(loaded.colliders.is_empty());
}

#[test]
fn sidecar_persists_mesh_collider_choice() {
    let path = tmp("collider_quad.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let mut settings = import_and_sync_sidecar(&path)
        .map(|a| sidecar::load(&path).unwrap().with_sub_objects(&a))
        .unwrap();
    settings
        .colliders
        .insert("Quad".to_string(), MeshColliderKind::Convex);
    sidecar::save(&path, &settings).unwrap();

    let loaded = sidecar::load(&path).unwrap();
    assert_eq!(
        loaded.colliders.get("Quad"),
        Some(&MeshColliderKind::Convex)
    );
    assert!(loaded.colliders.get("Quad").unwrap().is_convex());
}
