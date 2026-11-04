//! The fallback reflection probe (#1697, Unity's default reflection probe): a metal
//! sphere outside every probe box reflects the probe the scene names as its fallback
//! (a green room far away), not the sky (black here). With none named it reflects
//! the sky, as before.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use crate::gpu::reflection_probe_per_object_screenshot::solid_cube_ktx2;
use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 48;

/// A black sky, a metal sphere at the origin, and a green probe whose box is 50 m
/// away, the fallback when `fallback` is set.
fn scene(fallback: bool) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let metal = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness: 0.2,
        ..MaterialAsset::default()
    };
    scene.materials.insert("metal".to_string(), metal);
    let id = create_entity(&mut scene, "Sphere", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(2.0);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("metal".to_string())));

    let cube = crate::temp::dir().join("rusty_gpu_1697_fallback_green.ktx2");
    std::fs::write(&cube, solid_cube_ktx2(4, [0.0, 1.0, 0.0])).unwrap();
    let probes = &mut scene.reflection_probes;
    let i = probes.add(Vec3::new(50.0, 0.0, 0.0), Vec3::ONE);
    probes.edit(i, |p| p.blend_distance = 0.0);
    probes.set_cubemap(i, cube.to_string_lossy().into_owned());
    if fallback {
        assert!(probes.set_fallback(Some(i)));
    }
    scene
}

/// Mean green of the sphere's centre block; `None` without an adapter.
fn green(scene: &Scene, name: &str) -> Option<f64> {
    let camera = Camera::new(Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_1697_fallback_{name}.png"));
    capture(scene, &camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    let img = image::open(&path).unwrap().to_rgb8();
    let (lo, hi) = (SIZE / 2 - 4, SIZE / 2 + 4);
    let sum: f64 = (lo..hi)
        .flat_map(|y| (lo..hi).map(move |x| (x, y)))
        .map(|(x, y)| f64::from(img.get_pixel(x, y).0[1]))
        .sum();
    Some(sum / f64::from((hi - lo) * (hi - lo)))
}

#[test]
fn an_object_outside_every_box_reflects_the_fallback_not_the_sky() {
    let Some(with) = green(&scene(true), "named") else {
        eprintln!("[fallback] no GPU/software adapter — skipping");
        return;
    };
    let without = green(&scene(false), "none").unwrap();
    eprintln!("[fallback] green with fallback={with:.2}, without={without:.2}");
    assert!(
        with > 40.0,
        "the sphere reflects the green fallback: {with:.2}"
    );
    assert!(
        without < 10.0,
        "with none it reflects the black sky: {without:.2}"
    );
}
