//! A height map whose shift is zero filters every other map as a material without one.
//! Reading displaced maps at explicit derivatives lost the sampler's anisotropy on lavapipe (it
//! filters an explicit-gradient read isotropically): far, grazing detail drew sharper and shimmered.
//! Smallest case: a 0/1 stripe albedo, 32 periods a tile, seen at a grazing angle.

use glam::Vec3;
use rusty::components::MaterialAsset;
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;

use crate::gpu::linear_data_maps_screenshot::{diff, SAME};
use crate::gpu::normal_emissive_maps_screenshot::scene;

fn png(name: &str, striped: bool) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(256, 256, |x, _| match striped && (x / 4) % 2 == 1 {
        true => image::Rgba([0, 0, 0, 255]),
        false => image::Rgba([255, 255, 255, 255]),
    });
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

fn striped(height: Option<(bool, f32)>) -> MaterialAsset {
    let height_map = height.map(|(s, _)| png(&format!("rusty_2301_height_{s}.png"), s).into());
    MaterialAsset {
        base_color_map: Some(png("rusty_2301_stripes.png", true).into()),
        roughness: 1.0,
        tiling: [4.0, 4.0],
        height_map,
        height_scale: height.map_or(0.0, |(_, scale)| scale),
        ..MaterialAsset::default()
    }
}

fn frame(material: MaterialAsset, name: &str) -> Option<image::RgbImage> {
    let cam = Camera::new(Vec3::new(-7.5, 0.5, 0.0), 0.0, -4.0);
    let path = crate::temp::dir().join(name);
    capture(&scene(material), &cam, &path, 256, 128)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).expect("open").to_rgb8())
}

#[test]
fn a_height_map_that_shifts_nothing_filters_as_none_does() {
    let flat = frame(striped(None), "rusty_2301_none.png");
    let level = frame(striped(Some((false, 0.1))), "rusty_2301_level.png");
    let tiny = frame(striped(Some((true, 3e-5))), "rusty_2301_tiny.png");
    let (Some(flat), Some(level), Some(tiny)) = (flat, level, tiny) else {
        eprintln!("[height-map] no GPU adapter — skipping");
        return;
    };
    let (zero, small) = (diff(&flat, &level), diff(&flat, &tiny));
    eprintln!("[height-map] against no map: level {zero:.3}, scale 3e-5 {small:.3}");
    // Measured: lavapipe level 0.000, scale 3e-5 0.344 (4.126 with explicit derivatives); RTX 2060
    // 0.000 and 0.501. The scale's residue is the march's real sub-texel shift, 4° stretching 3e-5
    // to a texel.
    assert!(zero < 0.05, "a level height map shifts nothing: {zero:.3}");
    assert!(
        small < SAME,
        "a negligible scale filters as no map: {small:.3}"
    );
}
