use super::*;
use rusty_test_support::models::{SKINNED_GLTF, TEXTURED_GLTF, TRIANGLE_GLTF, TRIANGLE_OBJ};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let mut p = rusty_test_support::temp::dir();
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
    assert_eq!(tri.first_material(), Some(0));
    assert_eq!(asset.materials[0].name, "Red");
    assert_eq!(asset.materials[0].base_color, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn gltf_reads_full_metallic_roughness_material() {
    // Own dir so the resolved texture path is predictable; the importer never reads the file, so none is written.
    let dir = rusty_test_support::temp::dir().join("rusty_textured_fixture");
    std::fs::create_dir_all(&dir).unwrap();
    let gltf_path = dir.join("model.gltf");
    std::fs::write(&gltf_path, TEXTURED_GLTF).unwrap();

    let asset = import_file(&gltf_path).unwrap();
    let mat = &asset.materials[0];
    assert_eq!(mat.name, "Bricks");
    assert_eq!(mat.base_color, [0.8, 0.4, 0.2, 1.0]);
    assert_eq!(mat.metallic, 0.25);
    assert_eq!(mat.roughness, 0.75);
    assert_eq!(mat.emissive, [0.1, 0.2, 0.3]);
    let expected = dir
        .join("tex")
        .join("brick wall.png")
        .to_string_lossy()
        .into_owned();
    assert_eq!(mat.base_color_map, Some(expected.into()));
    assert!(mat.metallic_roughness_map.is_none());
    assert!(mat.normal_map.is_none());
    assert!(mat.emissive_map.is_none());
}

#[test]
fn gltf_embedded_material_uses_spec_defaults() {
    // Only a baseColorFactor: metallic/roughness fall to the glTF defaults (1.0).
    let path = tmp("defaults.gltf");
    std::fs::write(&path, TRIANGLE_GLTF).unwrap();
    let mat = &import_file(&path).unwrap().materials[0];
    assert_eq!(mat.metallic, 1.0);
    assert_eq!(mat.roughness, 1.0);
    assert!(mat.base_color_map.is_none());
    assert!(mat.metallic_roughness_map.is_none());
}

#[test]
fn gltf_imports_skin_and_bind_pose_palette() {
    let dir = rusty_test_support::temp::dir().join("rusty_skin_fixture");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("model.bin"),
        rusty_test_support::models::skinned_buffer(),
    )
    .unwrap();
    let gltf_path = dir.join("model.gltf");
    std::fs::write(&gltf_path, SKINNED_GLTF).unwrap();

    let asset = import_file(&gltf_path).unwrap();
    let sub = asset.sub_mesh("Skinned").unwrap();

    assert_eq!(sub.vertices.len(), 3);
    assert_eq!(sub.vertices[0].joint_indices, [0, 0, 0, 0]);
    assert_eq!(sub.vertices[1].joint_indices, [1, 0, 0, 0]);
    assert_eq!(sub.vertices[2].joint_indices, [0, 1, 0, 0]);
    assert_eq!(sub.vertices[2].joint_weights, [0.5, 0.5, 0.0, 0.0]);

    let skin = sub.skin.as_ref().expect("skinned sub-mesh has a skeleton");
    assert_eq!(skin.inverse_bind.len(), 2);
    assert_eq!(skin.bind_global.len(), 2);

    // Each joint's global is cancelled by its inverse bind, so the palette is identity.
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
fn gltf_imports_named_animation_clip() {
    use rusty_test_support::models::anim::{animated_buffer, ANIMATED_GLTF};
    let dir = rusty_test_support::temp::dir().join("rusty_anim_fixture");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("anim.bin"), animated_buffer()).unwrap();
    let gltf_path = dir.join("model.gltf");
    std::fs::write(&gltf_path, ANIMATED_GLTF).unwrap();

    let asset = import_file(&gltf_path).unwrap();
    let sub = asset.sub_mesh("Animated").unwrap();

    assert_eq!(sub.clips.len(), 1);
    let clip = &sub.clips[0];
    assert_eq!(clip.name, "Wave");
    assert_eq!(clip.tracks.len(), 2);
    assert!((clip.duration - 1.0).abs() < 1e-6);

    // Joint0 keyframes: [0,1] s -> [(0,0,0),(2,0,0)].
    let t = &clip.tracks[0].translation;
    assert_eq!(t.times, vec![0.0, 1.0]);
    assert_eq!(t.values[1], glam::Vec3::new(2.0, 0.0, 0.0));
    assert!(clip.tracks[1].is_empty());
}

#[test]
fn obj_has_no_clips() {
    let path = tmp("noskin_clips.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let sub = import_file(&path).unwrap().sub_meshes.remove(0);
    assert!(sub.clips.is_empty());
}

#[test]
fn obj_has_no_skin() {
    let path = tmp("noskin.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let sub = import_file(&path).unwrap().sub_meshes.remove(0);
    assert!(sub.skin.is_none());
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
        path: "assets/models/crates.glb".to_string(),
        sub_object: "Barrel".to_string(),
    };
    let s = r.to_string_ref();
    assert_eq!(s, "assets/models/crates.glb::Barrel");
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
        import_file(std::path::Path::new("foo.max")),
        Err(ImportError::UnsupportedFormat(_))
    ));
    assert!(is_model_path("a/b/c.GLB"));
    assert!(!is_model_path("a/b/c.png"));
    assert!(is_image_path("a/b/c.JPG") && !is_image_path("a/b/c.glb"));
    // A missing file is a parse error, not an unsupported one.
    for ext in MODEL_EXTENSIONS.iter().chain(&["max"]) {
        let err = import_file(&std::path::Path::new("missing").with_extension(ext));
        let unsupported = matches!(err, Err(ImportError::UnsupportedFormat(_)));
        assert_eq!(unsupported, !MODEL_EXTENSIONS.contains(ext), "{ext}");
    }
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
    assert!(loaded.colliders.is_empty());
}

#[test]
fn sidecar_persists_mesh_collider_choice() {
    let path = tmp("collider_quad.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    let mut settings = import_and_sync_sidecar(&path)
        .map(|a| sidecar::load(&path).unwrap().with_sub_objects(&a))
        .unwrap();
    settings.colliders.insert(
        "Quad".to_string(),
        SubObjectCollider::own(MeshColliderKind::Convex),
    );
    sidecar::save(&path, &settings).unwrap();

    let loaded = sidecar::load(&path).unwrap();
    assert_eq!(
        loaded.colliders.get("Quad").map(|c| c.kind),
        Some(MeshColliderKind::Convex)
    );
}

#[test]
fn concurrent_imports_never_read_a_half_written_sidecar() {
    // A non-atomic save truncates first, so a concurrent reader sees an empty file.
    let path = tmp("concurrent_quad.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    // Start with no sidecar: a leftover would decide this run.
    std::fs::remove_file(sidecar::meta_path(&path)).ok();
    for round in 0..20 {
        let failures: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..8)
                .map(|_| s.spawn(|| import_and_sync_sidecar(&path).err()))
                .collect();
            handles
                .into_iter()
                .filter_map(|h| match h.join() {
                    Ok(result) => result.map(|e| e.to_string()),
                    Err(_) => Some("thread panicked".to_string()),
                })
                .collect()
        });
        assert!(
            failures.is_empty(),
            "round {round}: imports failed: {failures:?}"
        );
    }
}
