//! The environment reflection's weight is URP's environment BRDF (#1543), not a
//! `1 - roughness` scale: a rough metal reflects a blurry, dimmer environment, never
//! a black one, and a dielectric reflects its 4% F0 head-on.
//!
//! The sky is a uniform grey panorama and the scene has no lights, no ambient and no
//! post-processing volume (so no tonemap): the only light on a sphere is the
//! environment reflection, and the frame is the reflection's weight times the sky.
//! Both are read back as linear light (the sRGB encoding undone): the centre of the
//! sphere (normal incidence) against a corner of the frame (the sky itself). Without
//! an adapter the capture returns `Ok(false)` and the test skips.

use std::path::PathBuf;

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

/// A uniform grey panorama, bright enough that a 4% reflection stays above the
/// 8-bit floor and dark enough that a full one does not clip.
fn grey_panorama() -> PathBuf {
    let img = image::RgbaImage::from_pixel(64, 32, image::Rgba([200, 200, 200, 255]));
    let path = crate::temp::dir().join("grey_panorama.png");
    img.save(&path).expect("write panorama");
    path
}

/// One sphere filling the frame's centre, of `material`, under the grey sky.
fn scene(material: MaterialAsset) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = grey_panorama().to_string_lossy().into_owned();
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Sphere", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(3.0);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("m".to_string())));
    scene.materials.insert("m".to_string(), material);
    scene
}

fn linear(srgb: u8) -> f64 {
    let c = srgb as f64 / 255.0;
    match c <= 0.04045 {
        true => c / 12.92,
        false => ((c + 0.055) / 1.055).powf(2.4),
    }
}

/// Mean linear green of the 8x8 block at `(x0, y0)`.
fn block(img: &image::RgbImage, x0: u32, y0: u32) -> f64 {
    let mut sum = 0.0;
    for y in y0..y0 + 8 {
        for x in x0..x0 + 8 {
            sum += linear(img.get_pixel(x, y)[1]);
        }
    }
    sum / 64.0
}

/// The sphere's reflection weight head-on: its centre over the sky, both linear.
/// `None` when no adapter rendered it.
fn reflectance(label: &str, material: MaterialAsset) -> Option<f64> {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_env_brdf_{label}.png"));
    let mut host = CaptureHost::new();
    if !capture_into(&mut host, &scene(material), &cam, &path, 96, 96).expect("capture") {
        eprintln!("[env_brdf] no GPU/software adapter — skipping");
        return None;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let (centre, sky) = (block(&img, 44, 44), block(&img, 0, 0));
    eprintln!("[env_brdf] {label}: centre {centre:.4} sky {sky:.4}");
    assert!(sky > 0.3, "{label}: the grey sky did not render ({sky:.4})");
    Some(centre / sky)
}

fn metal(roughness: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness,
        ..MaterialAsset::default()
    }
}

#[test]
fn rough_metal_reflects_a_dimmer_environment_not_a_black_one() {
    let Some(smooth) = reflectance("smooth", metal(0.1)) else {
        return;
    };
    let rough = reflectance("rough", metal(0.9)).expect("the adapter rendered once");
    // URP's `1 / (roughness^2 + 1)` keeps about 60% at perceptual roughness 0.9;
    // the old `1 - roughness` scale kept about 11%.
    assert!(
        smooth > 0.85,
        "smooth white metal reflects ~all of the sky: {smooth:.3}"
    );
    assert!(
        rough > 0.5 * smooth,
        "rough metal goes dark: {rough:.3} of the sky against smooth {smooth:.3}"
    );
    assert!(
        rough < smooth,
        "a rough lobe loses some energy: {rough:.3} vs {smooth:.3}"
    );
}

#[test]
fn a_black_dielectric_reflects_four_percent_head_on() {
    let plastic = MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        metallic: 0.0,
        roughness: 0.1,
        ..MaterialAsset::default()
    };
    let Some(f0) = reflectance("dielectric", plastic) else {
        return;
    };
    assert!(
        (0.025..0.06).contains(&f0),
        "dielectric F0 should be ~0.04: {f0:.4}"
    );
}
