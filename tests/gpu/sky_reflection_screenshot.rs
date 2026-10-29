//! The sky reflection is prefiltered (#1553): with no reflection probe, a rough
//! metal reflects a blurred sky and a mirror a sharp one, as Unity's skybox
//! reflection does. A sphere seen from above reflects the upper sky: banded rings
//! of a striped panorama, or the procedural sky's clouds. The sphere's middle is
//! read as linear light; its spread (standard deviation over mean, so the rough
//! lobe's lower energy does not count as blur) must fall with roughness.
//! Without an adapter the capture returns `Ok(false)` and the tests skip.

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

/// Eight dark and light bands from zenith to nadir.
fn banded_panorama() -> String {
    let img = image::RgbaImage::from_fn(64, 64, |_, y| match (y / 8) % 2 {
        0 => image::Rgba([230, 230, 230, 255]),
        _ => image::Rgba([30, 30, 30, 255]),
    });
    let path = crate::temp::dir().join("sky_reflection_bands.png");
    img.save(&path).expect("write the panorama");
    path.to_string_lossy().into_owned()
}

/// A white metal sphere of `roughness` under `sky` (empty: the procedural sky),
/// no lights, no ambient.
fn scene(sky: &str, roughness: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = sky.to_string();
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Sphere", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(3.0);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("m".to_string())));
    let metal = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness,
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), metal);
    scene
}

fn linear(srgb: u8) -> f64 {
    let c = f64::from(srgb) / 255.0;
    match c <= 0.04045 {
        true => c / 12.92,
        false => ((c + 0.055) / 1.055).powf(2.4),
    }
}

/// The relative spread of the sphere's middle 32×32, seen from 5 units above.
/// `None` when no adapter rendered it.
fn spread(label: &str, scene: &Scene) -> Option<f64> {
    let cam = Camera::new(Vec3::new(0.0, 5.0, 0.0), -90.0, -89.9);
    let path = crate::temp::dir().join(format!("rusty_sky_reflection_{label}.png"));
    if !capture_into(&mut CaptureHost::new(), scene, &cam, &path, 96, 96).expect("capture") {
        eprintln!("[sky_reflection] no GPU/software adapter — skipping");
        return None;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let values: Vec<f64> = (32..64)
        .flat_map(|y| (32..64).map(move |x| (x, y)))
        .map(|(x, y)| linear(img.get_pixel(x, y)[1]))
        .collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let spread = var.sqrt() / mean.max(1e-6);
    eprintln!("[sky_reflection] {label}: mean {mean:.4} spread {spread:.4}");
    assert!(
        mean > 0.01,
        "{label}: the sphere reflects nothing ({mean:.4})"
    );
    Some(spread)
}

/// The smooth and the rough sphere's spread under `sky`.
fn pair(label: &str, sky: &str) -> Option<(f64, f64)> {
    let smooth = spread(&format!("{label}_smooth"), &scene(sky, 0.0))?;
    let rough = spread(&format!("{label}_rough"), &scene(sky, 1.0)).expect("rendered once");
    Some((smooth, rough))
}

#[test]
fn a_rough_metal_blurs_a_banded_panorama() {
    let Some((smooth, rough)) = pair("bands", &banded_panorama()) else {
        return;
    };
    assert!(smooth > 0.3, "the mirror shows the bands: {smooth:.3}");
    assert!(
        rough < 0.25 * smooth,
        "the rough sphere still shows the bands: {rough:.3} against {smooth:.3}"
    );
}

#[test]
fn a_rough_metal_blurs_the_procedural_clouds() {
    let Some((smooth, rough)) = pair("clouds", "") else {
        return;
    };
    assert!(smooth > 0.05, "the mirror shows the clouds: {smooth:.3}");
    assert!(
        rough < 0.5 * smooth,
        "the rough sphere still shows the clouds: {rough:.3} against {smooth:.3}"
    );
}
