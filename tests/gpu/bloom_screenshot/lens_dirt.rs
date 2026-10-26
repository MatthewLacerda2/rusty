//! Bloom's lens dirt (#1442): a white dirt texture is lit only where the bloom
//! reaches it, and adds exactly nothing where there is no bloom, with bloom off, or
//! at a dirt intensity of `0`; and it is cropped to cover the view, not stretched.
//!
//! Skips without a GPU/software adapter, unless one is required.

use std::path::{Path, PathBuf};

use glam::Vec3;
use image::RgbImage;
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, Scene};

/// `scene` (bloom on or off, its box glowing or not) with a dirt texture of
/// `dirt` intensity, or none, rendered at `w` × `h`.
fn render_at(
    scene: Scene,
    dirt: Option<(&Path, f32)>,
    (w, h): (u32, u32),
    name: &str,
) -> Option<RgbImage> {
    let mut scene = scene;
    if let Some((file, k)) = dirt {
        for id in scene.world.ids_with_post_processing() {
            let mut vc = scene.world.post_processing_mut(id).unwrap();
            vc.bloom.dirt_texture = Some(file.to_string_lossy().into_owned());
            vc.bloom.dirt_intensity = k;
        }
    }
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_lens_dirt_{name}.png"));
    if !capture(&scene, &cam, &path, w, h).expect("capture") {
        assert!(
            !rusty::render::gpu_required(),
            "no adapter, but one is required"
        );
        return None;
    }
    Some(image::open(&path).unwrap().to_rgb8())
}

/// [`render_at`] 64², with the white dirt.
fn render(bloom: bool, glow: bool, dirt: Option<f32>, name: &str) -> Option<RgbImage> {
    let mut scene = super::scene(bloom);
    if !glow {
        scene.materials.get_mut("Glow").unwrap().emissive = [0.0; 3];
    }
    let white = dirt_file("white", |_| true);
    render_at(scene, dirt.map(|k| (white.as_path(), k)), (64, 64), name)
}

/// A 16² dirt texture, white where `white(row)`, black elsewhere.
fn dirt_file(name: &str, white: impl Fn(u32) -> bool) -> PathBuf {
    let file = crate::temp::dir().join(format!("rusty_lens_dirt_{name}.png"));
    let texel = |y| image::Rgba(if white(y) { [255; 4] } else { [0, 0, 0, 255] });
    image::RgbaImage::from_fn(16, 16, |_, y| texel(y))
        .save(&file)
        .unwrap();
    file
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

#[test]
fn lens_dirt_is_cropped_to_cover_a_wider_view() {
    // On a 2:1 view a square dirt shows its middle half: screen rows 7..11, lit by
    // the glow, read the dirt's rows 5..7 cropped, but its rows 3..5 stretched.
    let banded = dirt_file("banded", |y| !(5..11).contains(&y));
    let white = dirt_file("white", |_| true);
    let size = (64, 32);
    let Some(clean) = render_at(super::scene(true), None, size, "wide_clean") else {
        return;
    };
    let lit = render_at(super::scene(true), Some((&white, 4.0)), size, "wide_white").unwrap();
    let band = gain(&clean, &lit, (24, 7), 4);
    assert!(
        band > 5.0,
        "the glow must light rows 7..11 (gain {band:.2})"
    );
    let got = render_at(
        super::scene(true),
        Some((&banded, 4.0)),
        size,
        "wide_banded",
    )
    .unwrap();
    assert!(
        got == clean,
        "the dirt's cropped rows showed: it was stretched"
    );
}
