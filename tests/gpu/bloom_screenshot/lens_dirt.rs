//! Bloom's lens dirt (#1442): a white dirt texture is lit only where the bloom
//! reaches it, and adds exactly nothing where there is no bloom, with bloom off, or
//! at a dirt intensity of `0`.
//!
//! Skips without a GPU/software adapter, unless one is required.

use glam::Vec3;
use image::RgbImage;
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, Scene};

/// `scene` (bloom on or off, its box glowing or not) with the white dirt at
/// `dirt` intensity, or none, rendered at 64².
fn render(bloom: bool, glow: bool, dirt: Option<f32>, name: &str) -> Option<RgbImage> {
    let mut scene = super::scene(bloom);
    if !glow {
        scene.materials.get_mut("Glow").unwrap().emissive = [0.0; 3];
    }
    if let Some(k) = dirt {
        set_dirt(&mut scene, k);
    }
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_lens_dirt_{name}.png"));
    if !capture(&scene, &cam, &path, 64, 64).expect("capture") {
        assert!(
            !rusty::render::gpu_required(),
            "no adapter, but one is required"
        );
        return None;
    }
    Some(image::open(&path).unwrap().to_rgb8())
}

/// Point every volume at a white dirt texture of strength `k`.
fn set_dirt(scene: &mut Scene, k: f32) {
    let file = crate::temp::dir().join("rusty_lens_dirt_white.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255; 4]))
        .save(&file)
        .unwrap();
    for id in scene.world.ids_with_post_processing() {
        let mut vc = scene.world.post_processing_mut(id).unwrap();
        vc.bloom.dirt_texture = Some(file.to_string_lossy().into_owned());
        vc.bloom.dirt_intensity = k;
    }
}

/// The mean brightness `b` adds over `a` in the square of side `side` at `(x, y)`.
fn gain(a: &RgbImage, b: &RgbImage, (x, y): (u32, u32), side: u32) -> f64 {
    let mut sum = 0.0;
    for (px, py) in (x..x + side).flat_map(|px| (y..y + side).map(move |py| (px, py))) {
        let (pa, pb) = (a.get_pixel(px, py).0, b.get_pixel(px, py).0);
        sum += (0..3).map(|c| pb[c] as f64 - pa[c] as f64).sum::<f64>() / 3.0;
    }
    sum / (side * side) as f64
}

#[test]
fn lens_dirt_lights_up_only_around_a_bright_source() {
    let Some(clean) = render(true, true, None, "clean") else {
        return;
    };
    let dirty = render(true, true, Some(4.0), "dirty").unwrap();
    let (near, far) = (
        gain(&clean, &dirty, (12, 12), 40),
        gain(&clean, &dirty, (0, 0), 6),
    );
    eprintln!("[lens dirt] near={near:.2} far={far:.2}");
    assert!(
        near > 5.0,
        "the dirt must light up around the glow (gain {near:.2})"
    );
    assert!(
        far < near / 4.0,
        "the dirt stays dark away from it ({far:.2} vs {near:.2})"
    );
}

#[test]
fn lens_dirt_adds_nothing_without_bloom_light() {
    let pairs = [
        // No bright source: nothing blooms, so no dirt shows.
        ((true, false, None), (true, false, Some(4.0))),
        // Bloom off: no dirt, whatever the knob says.
        ((false, true, None), (false, true, Some(4.0))),
        // A dirt intensity of 0 is the frame before lens dirt existed.
        ((true, true, None), (true, true, Some(0.0))),
    ];
    for (i, ((b0, g0, d0), (b1, g1, d1))) in pairs.into_iter().enumerate() {
        let Some(want) = render(b0, g0, d0, &format!("want{i}")) else {
            return;
        };
        let got = render(b1, g1, d1, &format!("got{i}")).unwrap();
        assert!(want == got, "case {i}: the dirt changed the frame");
    }
}
