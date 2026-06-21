//! Emissive-factor visual test (#222). Proves the renderer READS a material's flat
//! `emissive` factor and adds it to the lit colour through the per-entity material
//! uniform (`EntityUniform`/`EntityUniforms`), not just stores it on the CPU.
//!
//! The signal is light-independent by construction: the scene has **zero ambient and
//! no lights**, so the only thing that can brighten the surface is the emissive term
//! in `fs_main`. A black-base box with `emissive = 0` renders near-black; the same box
//! with a bright emissive factor renders bright. A factor >1.0 additionally exercises
//! the HDR target + bloom bright-pass, so its mean brightness clears the `< 1.0` one.
//!
//! When no GPU/software adapter is present, `capture` returns `Ok(false)` and the test
//! passes without asserting (matching `material_maps_screenshot.rs`).

#![cfg(feature = "dev")]

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent, Tonemap};
use rusty::dev::screenshot::capture;
use rusty::render::Camera;
use rusty::scene::{MeshComponent, Scene, VisualCorrectionComponent};

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

fn vc(bloom: bool) -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        active: true,
        bloom_active: bloom,
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

/// An unlit-by-construction box (ambient 0, no lights) whose only brightness source is
/// the material's `emissive` factor. Base color is black so nothing else contributes.
fn scene(emissive: [f32; 3], bloom: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;

    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::render::mesh::generate_box(8.0, 8.0, 8.0);
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
    e.visual_correction = Some(vc(bloom));
    drop(e);
    scene.materials.insert(
        "mat".to_string(),
        MaterialAsset {
            base_color: [0.0, 0.0, 0.0],
            emissive,
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
fn emissive_factor_visibly_lights_an_otherwise_dark_surface() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);

    let p_off = tmp("rusty_emissive_off.png");
    let p_dim = tmp("rusty_emissive_dim.png");
    let p_bloom = tmp("rusty_emissive_bloom.png");

    let c0 = capture(&scene([0.0, 0.0, 0.0], false), &cam, &p_off, 96, 96).expect("capture");
    let c1 = capture(&scene([0.6, 0.6, 0.6], false), &cam, &p_dim, 96, 96).expect("capture");
    let c2 = capture(&scene([6.0, 6.0, 6.0], true), &cam, &p_bloom, 96, 96).expect("capture");
    if !c0 || !c1 || !c2 {
        eprintln!("[emissive] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let off = mean_brightness(&p_off).expect("png");
    let dim = mean_brightness(&p_dim).expect("png");
    let bloom = mean_brightness(&p_bloom).expect("png");
    eprintln!("[emissive] off={off:.3} dim={dim:.3} bloom={bloom:.3}");

    assert!(
        dim > off + 2.0,
        "emissive factor must brighten an unlit surface (off={off:.3}, dim={dim:.3})"
    );
    assert!(
        bloom > dim + 2.0,
        "a >1.0 emissive factor must clear a <1.0 one via the HDR/bloom path \
         (bloom={bloom:.3}, dim={dim:.3})"
    );
}
