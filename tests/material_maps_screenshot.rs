//! Material-map visual test (issue #184 / #202). Proves the renderer actually
//! SAMPLES a material's map textures — not just its scalar factors — through the
//! per-entity material bind group added in #202.
//!
//! It drives the **metallic map**, because metallic has a strong, view- and
//! light-geometry-independent effect: the shader gates the ambient diffuse term by
//! `(1 - metallic)` (see `shader.wgsl`), so a fully-metallic surface under ambient
//! light is dark while a non-metallic one is bright. Holding the scalar at
//! `metallic = 1.0` and only varying the map isolates the map's contribution:
//! `no map` -> metallic 1.0 -> dark; `black map (.b = 0)` -> 1.0 * 0.0 = metallic 0
//! -> bright. If the map were not loaded/sampled it would fall back to the default
//! white texture (`.b = 1`), leaving metallic at 1.0 and the image unchanged — so
//! this difference is a genuine read-site proof. The roughness map travels the
//! identical path (same bind group, same `select()` in the shader, `.g` channel);
//! the WGSL binding wiring for both is validated GPU-free by `all_shaders_compose`.
//!
//! When no GPU/software adapter is present, `capture` returns `Ok(false)` and the
//! test passes without asserting (matching `postfx_screenshot.rs`).

#![cfg(feature = "dev")]

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent, Tonemap};
use rusty::dev::screenshot::capture;
use rusty::render::Camera;
use rusty::scene::{MeshComponent, Scene, VisualCorrectionComponent};

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

/// Write a 2x2 black RGBA PNG. Metallic is read from the BLUE channel (glTF
/// metallic-roughness convention), so `.b = 0` drives metallic to zero.
fn write_black_png(path: &std::path::Path) {
    let mut img = image::RgbaImage::new(2, 2);
    for px in img.pixels_mut() {
        *px = image::Rgba([0, 0, 0, 255]);
    }
    img.save(path).expect("write metallic png");
}

/// Neutral post-FX so both frames tonemap identically (only the map differs).
fn vc() -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        active: true,
        bloom_active: false,
        bloom_intensity: 1.0,
        bloom_threshold: 0.6,
        exposure: 0.0,
        contrast: 1.0,
        saturation: 1.0,
        ssr_active: false,
        ssr_quality: "High".to_string(),
        ssr_temporal_upsampling: false,
        tonemap: Tonemap::Aces,
        gamma: 2.2,
    }
}

/// An ambient-lit box with a fixed `metallic = 1.0` scalar. With `metallic_map`
/// `Some`, the sampled blue channel scales that scalar; a black map drives metallic
/// to 0, restoring the ambient diffuse term.
fn scene_with_metallic_map(metallic_map: Option<String>) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;

    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::render::mesh::generate_box(2.0, 2.0, 2.0);
    let mut e = scene.get_entity_mut(id).unwrap();
    e.mesh = Some(MeshComponent {
        primitive_type: "Box".to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        is_dirty: rusty::scene::DirtyFlag::new(true),
    });
    e.material = Some(MaterialComponent {
        material: "mat".to_string(),
    });
    e.visual_correction = Some(vc());
    drop(e);
    scene.materials.insert(
        "mat".to_string(),
        MaterialAsset {
            base_color: [0.9, 0.9, 0.9],
            metallic: 1.0,
            roughness: 1.0,
            metallic_map,
            ..MaterialAsset::default()
        },
    );
    scene
}

fn mean_brightness(path: &std::path::Path) -> Option<f64> {
    let img = image::open(path).ok()?.to_rgb8();
    let total: u64 = img
        .pixels()
        .map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64)
        .sum();
    Some(total as f64 / (img.pixels().len() as f64 * 3.0))
}

#[test]
fn metallic_map_visibly_changes_the_rendered_image() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);

    let map_png = tmp("rusty_metallic_map.png");
    write_black_png(&map_png); // .b = 0 -> metallic 1.0 * 0.0 = 0.0

    let dark = tmp("rusty_metal_scalar.png");
    let bright = tmp("rusty_metal_mapped.png");

    let scalar_scene = scene_with_metallic_map(None); // metallic 1.0 -> dark
    let mapped_scene = scene_with_metallic_map(Some(map_png.to_string_lossy().into_owned()));

    let cap_a = capture(&scalar_scene, &cam, &dark, 96, 96).expect("capture must not error");
    let cap_b = capture(&mapped_scene, &cam, &bright, 96, 96).expect("capture must not error");

    if !cap_a || !cap_b {
        eprintln!("[material-maps] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let b_dark = mean_brightness(&dark).expect("scalar png readable");
    let b_bright = mean_brightness(&bright).expect("mapped png readable");
    eprintln!("[material-maps] mean brightness: metallic-scalar={b_dark:.3}, metallic-mapped={b_bright:.3}");
    assert!(
        b_bright > b_dark + 2.0,
        "the metallic MAP must change the image vs the scalar alone — a black map \
         drives metallic to 0 and restores ambient diffuse (scalar={b_dark:.3}, mapped={b_bright:.3})"
    );
}
