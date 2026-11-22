use glam::Vec3;

use super::gpu_tests::{scaled, upscales};
use crate::postfx::effects::taa::gpu::fixture::{eye, frame, renderer, taa_scene};
use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

/// Measured on lavapipe: 14.9 against 0.9.
const FACTOR: f64 = 2.0;

fn checker_map() -> String {
    let img = image::RgbaImage::from_fn(320, 320, |x, y| {
        let v = if (x + y) % 2 == 0 { 255 } else { 0 };
        image::Rgba([v, v, v, 255])
    });
    let path = rusty_test_support::temp::dir().join("rusty_1702_taau_checker.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn wall_scene(taa: bool) -> Scene {
    let (mut scene, _) = taa_scene(taa, false);
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, 0.0, -7.0), Vec3::splat(10.0));
    drop(t);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("m".to_string())));
    let asset = MaterialAsset {
        base_color: [0.0; 3],
        emissive: [1.0; 3],
        emissive_map: Some(checker_map().into()),
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), asset);
    scene
}

fn contrast(rgba: &[u8]) -> f64 {
    let luma: Vec<f64> = rgba.chunks(4).map(|p| p[1] as f64).collect();
    let mean = luma.iter().sum::<f64>() / luma.len() as f64;
    let var = luma.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / luma.len() as f64;
    var.sqrt()
}

#[test]
fn gpu_an_upscaled_fine_texture_reads_a_mip_finer_through_the_bias() {
    let Some(mut r) = renderer() else { return };
    let (scene, cam) = (wall_scene(true), eye(0.0));
    let mut half = scaled(&r, 0.5);
    let t = contrast(&frame(&mut r, &mut half, &scene, &cam));
    let off = wall_scene(false);
    let mut plain = scaled(&r, 0.5);
    let b = contrast(&frame(&mut r, &mut plain, &off, &cam));
    eprintln!("[taau] checker contrast at 0.5: biased {t:.1}, unbiased (bilinear) {b:.1}");
    assert!(upscales(&half), "0.5 upscales through TAA");
    assert!(
        t > FACTOR * b.max(1.0),
        "the biased lookup reads the finer mip: {t:.1} vs {b:.1}"
    );
}
