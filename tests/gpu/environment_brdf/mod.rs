//! The environment reflection's weight is URP's environment BRDF (#1543), not a
//! `1 - roughness` scale: a rough metal reflects a blurry, dimmer environment, never
//! a black one, and a dielectric reflects its 4% F0 head-on, from either side.
//!
//! The sky is a uniform grey panorama and the scene has no lights, no ambient and no
//! post-processing volume (so no tonemap): the only light on a surface is the
//! environment reflection, and the frame is the reflection's weight times the sky.
//! Both are read back as linear light (the sRGB encoding undone): the centre of the
//! frame (the surface, at normal incidence) against a corner (the sky itself).
//! Without an adapter the capture returns `Ok(false)` and the tests skip.

mod back_face_screenshot;
mod sphere_screenshot;

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

/// One `shape` of `material` under the grey sky, no other light; returns its id.
fn scene(shape: Primitive, material: MaterialAsset) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = grey_panorama().to_string_lossy().into_owned();
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Surface", Some(shape));
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("m".to_string())));
    scene.materials.insert("m".to_string(), material);
    (scene, id)
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

/// The reflection weight at the frame's centre, seen from `(0, 0, 5)` down -Z:
/// the centre over the sky, both linear. `None` when no adapter rendered it.
fn reflectance(label: &str, scene: &Scene) -> Option<f64> {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_env_brdf_{label}.png"));
    let mut host = CaptureHost::new();
    if !capture_into(&mut host, scene, &cam, &path, 96, 96).expect("capture") {
        eprintln!("[env_brdf] no GPU/software adapter — skipping");
        return None;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let (centre, sky) = (block(&img, 44, 44), block(&img, 0, 0));
    eprintln!("[env_brdf] {label}: centre {centre:.4} sky {sky:.4}");
    assert!(sky > 0.3, "{label}: the grey sky did not render ({sky:.4})");
    Some(centre / sky)
}

/// A black dielectric: all it shows is its specular reflection.
fn black_plastic(roughness: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        metallic: 0.0,
        roughness,
        ..MaterialAsset::default()
    }
}
