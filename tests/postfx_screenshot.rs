//! Post-FX visual smoke test (issue #14). When a GPU / software adapter (e.g.
//! lavapipe) is present, render the same scene twice with different exposure and
//! assert the average pixel brightness actually changes — i.e. the color-
//! correction knob reaches the image. When no adapter is available, `capture`
//! returns `Ok(false)` and the test passes without asserting (GPU verification
//! simply wasn't possible in this environment).

#![cfg(feature = "dev")]

use glam::Vec3;
use rusty::components::Tonemap;
use rusty::dev::screenshot::capture;
use rusty::render::Camera;
use rusty::scene::{MeshComponent, Scene, VisualCorrectionComponent};

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

fn vc(exposure: f32) -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        active: true,
        bloom_active: true,
        bloom_intensity: 1.0,
        bloom_threshold: 0.6,
        exposure,
        contrast: 1.0,
        saturation: 1.0,
        ssr_active: false,
        ssr_quality: "High".to_string(),
        ssr_temporal_upsampling: false,
        tonemap: Tonemap::Aces,
        gamma: 2.2,
    }
}

fn scene_with_exposure(exposure: f32) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;
    let id = scene.add_entity("Box".to_string());
    let mut e = scene.get_entity_mut(id).unwrap();
    let (vertices, indices) = rusty::render::mesh::generate_box(2.0, 2.0, 2.0);
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
    e.visual_correction = Some(vc(exposure));
    drop(e);
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
fn exposure_visibly_changes_the_rendered_image() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);

    let dark = tmp("rusty_postfx_dark.png");
    let bright = tmp("rusty_postfx_bright.png");

    let low = scene_with_exposure(-2.0);
    let high = scene_with_exposure(2.0);

    let captured_low = capture(&low, &cam, &dark, 96, 96).expect("capture must not error");
    let captured_high = capture(&high, &cam, &bright, 96, 96).expect("capture must not error");

    if !captured_low || !captured_high {
        eprintln!("[postfx] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let b_dark = mean_brightness(&dark).expect("dark png readable");
    let b_bright = mean_brightness(&bright).expect("bright png readable");
    eprintln!("[postfx] mean brightness: exposure -2 = {b_dark:.2}, +2 = {b_bright:.2}");
    assert!(
        b_bright > b_dark + 1.0,
        "higher exposure should brighten the image (dark={b_dark:.2}, bright={b_bright:.2})"
    );
}
