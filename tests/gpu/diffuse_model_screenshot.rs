//! One diffuse model on every path: a lamp shades a static sphere the same `Realtime`
//! (`calculate_pbr`) or `Baked` (lightmap trace), under `Lambert` and `HalfLambert`,
//! and the two models differ visibly.

use glam::Vec3;
use rusty::components::{LightComponent, LightMode};
use rusty::dev::bake::lightmap::bake_scene_lightmaps;
use rusty::dev::screenshot::capture;
use rusty::edit::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::settings::BakeSettings;
use rusty::scene::lighting::DiffuseModel;
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;

fn scene(model: DiffuseModel, mode: LightMode) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    scene.lighting_settings.diffuse_model = model;
    let sphere = scene.add_entity("Sphere".into());
    scene
        .world
        .set_mesh(sphere, primitive_mesh_component(Primitive::Sphere));
    scene.world.set_static(sphere, true);
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(1.5, 2.5, 0.0);
    let light = LightComponent {
        intensity: 20.0,
        range: 20.0,
        cast_shadows: false,
        mode,
        ..rusty::edit::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

fn shot(scene: &Scene, name: &str) -> Option<image::RgbImage> {
    let camera = Camera::new(Vec3::new(0.0, 1.8, 0.0), 0.0, -89.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_1698_{name}.png"));
    capture(scene, &camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).unwrap().to_rgb8())
}

fn baked_shot(model: DiffuseModel, name: &str) -> image::RgbImage {
    let mut scene = scene(model, LightMode::Baked);
    let dir = crate::temp::dir().join(format!("rusty_gpu_1698_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let settings = BakeSettings {
        texels_per_unit: 24.0,
        samples: 4,
        bounces: 1,
        denoiser: rusty::scene::lighting::settings::Denoiser::Filter,
        filter_radius: 0,
        ..BakeSettings::default()
    };
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &settings).unwrap(),
        1
    );
    shot(&scene, name).unwrap()
}

/// Mean absolute difference over the pixels `mask` lights clearly: the lamp-facing side short
/// of the terminator, whose last texels the lightmap filtering blends. Past it the wrap light
/// reaches a live surface but not a baked one, whose own mesh blocks the shadow ray
/// (`Lighting.md`, *Baked vs Realtime*): that is a shadow, not the diffuse model.
fn diff(a: &image::RgbImage, b: &image::RgbImage, mask: &image::RgbImage) -> f64 {
    let (mut sum, mut n) = (0.0, 0);
    for ((p, q), m) in a.pixels().zip(b.pixels()).zip(mask.pixels()) {
        if m.0.iter().any(|&c| c > 20) {
            sum +=
                p.0.iter()
                    .zip(q.0)
                    .map(|(&x, y)| (x as f64 - y as f64).abs())
                    .sum::<f64>()
                    / 3.0;
            n += 1;
        }
    }
    sum / n.max(1) as f64
}

#[test]
fn one_lamp_shades_alike_baked_or_realtime_under_each_diffuse_model() {
    let mut realtime: Vec<image::RgbImage> = Vec::new();
    for model in DiffuseModel::ALL {
        let name = model.name();
        let Some(live) = shot(&scene(model, LightMode::Realtime), &format!("{name}_live")) else {
            eprintln!("[diffuse] no GPU/software adapter — skipping");
            return;
        };
        let baked = baked_shot(model, &format!("{name}_baked"));
        let d = diff(&live, &baked, realtime.first().unwrap_or(&live));
        eprintln!(
            "[diffuse] {name}: live {:.1} baked {:.1} diff {d:.2}",
            crate::gpu::mean(&live),
            crate::gpu::mean(&baked)
        );
        assert!(
            crate::gpu::mean(&live) > 10.0,
            "{name}: the lamp lights the sphere"
        );
        assert!(
            d < 4.0,
            "{name}: realtime and baked differ by {d:.2} per channel"
        );
        realtime.push(live);
    }
    let between = diff(&realtime[0], &realtime[1], &realtime[0]);
    eprintln!("[diffuse] Lambert vs HalfLambert: {between:.2}");
    assert!(between > 8.0, "the wrap changes the look: {between:.2}");
}
