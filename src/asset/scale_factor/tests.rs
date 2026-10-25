//! The Scale Factor at import (#1398): a primitive model at 0.01 and 1, a skinned
//! one whose bones still line up with its scaled mesh, and an animated one whose
//! translation keys scale with it.

use glam::{Mat4, Vec3};

use super::{sanitize, MIN_SCALE};
use crate::asset::fixtures::{quad_mode_gltf, skinned_buffer, SKINNED_GLTF};
use crate::asset::fixtures_anim::{animated_buffer, ANIMATED_GLTF};
use crate::asset::{import_file, sidecar, ImportedAsset};

/// Write `files` into a fresh temp dir and import `main` from it at `scale`.
fn import_at(dir: &str, files: &[(&str, &[u8])], main: &str, scale: f32) -> ImportedAsset {
    let dir = crate::test_temp::dir().join(dir);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    let path = dir.join(main);
    sidecar::set_scale(&path, scale).unwrap();
    import_file(&path).unwrap()
}

fn quad_at(scale: f32) -> ImportedAsset {
    let (json, bin) = quad_mode_gltf(4, &[0, 1, 2, 0, 2, 3]);
    let files: [(&str, &[u8]); 2] = [("quad.gltf", json.as_bytes()), ("quad.bin", &bin)];
    import_at(
        &format!("rusty_scale_quad_{scale}"),
        &files,
        "quad.gltf",
        scale,
    )
}

fn skinned_at(scale: f32) -> ImportedAsset {
    let bin = skinned_buffer();
    let files: [(&str, &[u8]); 2] = [("model.gltf", SKINNED_GLTF.as_bytes()), ("model.bin", &bin)];
    import_at(
        &format!("rusty_scale_skin_{scale}"),
        &files,
        "model.gltf",
        scale,
    )
}

#[test]
fn a_primitive_model_imports_at_its_scale_factor() {
    for (scale, max) in [
        (1.0, Vec3::new(1.0, 1.0, 0.0)),
        (0.01, Vec3::new(0.01, 0.01, 0.0)),
    ] {
        let (min, got) = quad_at(scale).sub_meshes[0].bounds().unwrap();
        assert_eq!(min, Vec3::ZERO);
        assert!(got.abs_diff_eq(max, 1e-7), "scale {scale}: max {got}");
    }
}

#[test]
fn the_normals_stay_unit_length() {
    let sub = &quad_at(0.01).sub_meshes[0];
    assert!(sub.vertices.iter().all(|v| v.normal == [0.0, 0.0, 1.0]));
}

#[test]
fn a_skinned_models_bones_line_up_with_its_scaled_mesh() {
    let one = skinned_at(1.0);
    let small = skinned_at(0.01);
    let (one, small) = (&one.sub_meshes[0], &small.sub_meshes[0]);
    for (a, b) in one.vertices.iter().zip(&small.vertices) {
        let scaled = Vec3::from_array(a.position) * 0.01;
        assert!(Vec3::from_array(b.position).abs_diff_eq(scaled, 1e-7));
    }
    let skin = small.skin.as_ref().unwrap();
    // At bind pose every joint still cancels its inverse bind: the mesh is unmoved.
    for m in skin.bind_palette() {
        assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-5), "{m:?}");
    }
    // Joint1 sits at (1, 2, 0) authored, so at (0.01, 0.02, 0) scaled, and the rest
    // pose composed from the locals lands on the same spot.
    let joint1 = skin.bind_global[1].w_axis.truncate();
    assert!(
        joint1.abs_diff_eq(Vec3::new(0.01, 0.02, 0.0), 1e-7),
        "{joint1}"
    );
    let composed = skin.root_offset(0) * skin.local_bind[0].matrix() * skin.local_bind[1].matrix();
    assert!(composed.abs_diff_eq(skin.bind_global[1], 1e-6));
}

#[test]
fn an_animations_translation_keys_scale_with_the_model() {
    let bin = animated_buffer();
    let files: [(&str, &[u8]); 2] = [("model.gltf", ANIMATED_GLTF.as_bytes()), ("anim.bin", &bin)];
    let asset = import_at("rusty_scale_anim", &files, "model.gltf", 0.5);
    let track = &asset.sub_mesh("Animated").unwrap().clips[0].tracks[0].translation;
    assert_eq!(
        track.values[1],
        Vec3::new(1.0, 0.0, 0.0),
        "(2, 0, 0) halved"
    );
    assert_eq!(track.times, vec![0.0, 1.0], "the timing is untouched");
}

#[test]
fn a_bad_scale_factor_changes_nothing_and_a_tiny_one_is_floored() {
    assert_eq!(sanitize(f32::NAN, 0.5), 0.5);
    assert_eq!(sanitize(f32::INFINITY, 0.5), 0.5);
    assert_eq!(sanitize(0.0, 0.5), MIN_SCALE);
    assert_eq!(sanitize(-2.0, 0.5), MIN_SCALE);
    assert_eq!(sanitize(0.01, 1.0), 0.01);
}
