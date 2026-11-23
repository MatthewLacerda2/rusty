//! The height map's parallax occlusion: a 16 m plane of four repeats of a step, white/high then
//! black/low, so the black left in the frame shows where the march lands.

mod battens;
mod filtering;
mod mips;
mod planar;
mod plateau;
mod ridges;

use glam::Vec3;
use rusty::components::MaterialAsset;
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;

use crate::gpu::linear_data_maps_screenshot::{diff, SAME};
use crate::gpu::normal_emissive_maps_screenshot::scene;

const SIZE: u32 = 128;

fn stripes(name: &str) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(64, 64, |x, _| match x < 32 {
        true => image::Rgba([255, 255, 255, 255]),
        false => image::Rgba([0, 0, 0, 255]),
    });
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

fn stepped(tiling: f32, height: Option<f32>) -> MaterialAsset {
    let map = stripes("rusty_2191_steps.png");
    MaterialAsset {
        base_color_map: Some(map.clone().into()),
        roughness: 1.0,
        tiling: [tiling, tiling],
        height_map: height.map(|_| map.into()),
        height_scale: height.unwrap_or(0.0),
        ..MaterialAsset::default()
    }
}

fn grazing(degrees: f32) -> Camera {
    let back = 7.0;
    let up = back * degrees.to_radians().tan();
    Camera::new(Vec3::new(-back, up, 0.0), 0.0, -degrees)
}

fn frame(material: MaterialAsset, camera: &Camera, name: &str) -> Option<image::RgbImage> {
    let path = crate::temp::dir().join(name);
    capture(&scene(material), camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).expect("open").to_rgb8())
}

fn black_share(img: &image::RgbImage) -> f64 {
    let luma = |p: &image::Rgb<u8>| p.0.iter().map(|&c| u32::from(c)).sum::<u32>();
    let bright = img.pixels().map(luma).max().unwrap_or(0);
    let plane: Vec<u32> = img.pixels().map(luma).filter(|&l| l > 0).collect();
    let black = plane.iter().filter(|&&l| l * 4 < bright).count();
    black as f64 / plane.len().max(1) as f64
}

#[test]
fn the_upper_steps_hide_more_of_the_lower_as_the_view_grazes() {
    let mut drops = Vec::new();
    for degrees in [30.0, 15.0] {
        let cam = grazing(degrees);
        let name = |what: &str| format!("rusty_2191_{what}_{degrees}.png");
        let flat = frame(stepped(4.0, None), &cam, &name("flat"));
        let marched = frame(stepped(4.0, Some(0.1)), &cam, &name("pom"));
        let (Some(flat), Some(marched)) = (flat, marched) else {
            eprintln!("[height-map] no GPU adapter — skipping");
            return;
        };
        let (a, b) = (black_share(&flat), black_share(&marched));
        eprintln!("[height-map] {degrees}°: black share flat {a:.3}, marched {b:.3}");
        drops.push(a - b);
    }
    // Drops in the plane's black share; lavapipe 0.065 at 30°, 0.150 at 15°.
    assert!(
        drops[0] > 0.02,
        "the lower steps are partly hidden: {drops:?}"
    );
    assert!(drops[1] > drops[0], "and more at a lower angle: {drops:?}");
}

#[test]
fn head_on_at_zero_scale_and_without_a_map_it_draws_flat() {
    let top = Camera::new(Vec3::new(0.0, 12.0, 0.0), 0.0, -90.0);
    let cases = [
        ("flat", stepped(4.0, None), &top),
        ("head_on", stepped(4.0, Some(0.02)), &top),
        ("zero", stepped(4.0, Some(0.0)), &grazing(15.0)),
        ("zero_flat", stepped(4.0, None), &grazing(15.0)),
        ("missing", missing_map(), &grazing(15.0)),
    ];
    let frames: Option<Vec<_>> = (cases.into_iter())
        .map(|(what, m, cam)| frame(m, cam, &format!("rusty_2191_{what}.png")))
        .collect();
    let Some(f) = frames else {
        eprintln!("[height-map] no GPU adapter — skipping");
        return;
    };
    let (head_on, zero) = (diff(&f[0], &f[1]), diff(&f[2], &f[3]));
    eprintln!("[height-map] head-on {head_on:.3}, scale 0 {zero:.3}");
    // Measured head-on on lavapipe: 0.882.
    assert!(head_on < SAME, "head-on it matches the flat: {head_on:.3}");
    assert_eq!(zero, 0.0, "a scale of 0 draws byte-identically");
    assert_eq!(diff(&f[3], &f[4]), 0.0, "so does a map with no texture");
}

fn missing_map() -> MaterialAsset {
    let path = crate::temp::dir().join("rusty_2191_no_such_height.png");
    MaterialAsset {
        height_map: Some(path.to_string_lossy().into_owned().into()),
        height_scale: 0.2,
        ..stepped(4.0, None)
    }
}

#[test]
fn past_the_fade_distance_it_draws_flat() {
    // 1024 repeats over 16 m: even the nearest row reads past `LOD_THRESHOLD` (mip 5, HDRP's
    // default) and its one-mip fade, so nothing marches.
    let cam = grazing(15.0);
    let flat = frame(stepped(1024.0, None), &cam, "rusty_2191_far_flat.png");
    let marched = frame(stepped(1024.0, Some(0.2)), &cam, "rusty_2191_far_pom.png");
    let (Some(flat), Some(marched)) = (flat, marched) else {
        eprintln!("[height-map] no GPU adapter — skipping");
        return;
    };
    let d = diff(&flat, &marched);
    eprintln!("[height-map] past the fade: {d:.3}");
    // Measured on lavapipe: 0.000.
    assert!(d < SAME, "a far surface is not displaced: {d:.3}");
}
