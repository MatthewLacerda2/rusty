use glam::{Quat, Vec3};

use super::fixture::{eye, frame, renderer, taa_scene, view};
use rusty_asset::sidecar::{FilterMode, TextureSettings};
use rusty_components::{LightComponent, MaterialAsset, MaterialComponent};
use rusty_scene::create::{create_entity, default_light, Primitive};
use rusty_scene::Scene;

const WARM: u32 = 8;
const MEASURED: u32 = 12;
/// Measured: FXAA 0.0405, TAA 0.0170 on lavapipe and an RTX 2060; 0.0399, 0.0170 on Metal.
const FACTOR: f64 = 1.5;
const STEP: f32 = 1.0;

/// Unmipped: mipped, the sparkle is the driver's anisotropic filtering and LOD, which differ
/// by adapter.
fn bumpy_normal_map() -> String {
    let mut state = 0x9e37_79b9u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 24) as u8
    };
    let img = image::RgbaImage::from_fn(64, 64, |_, _| {
        let (x, y) = (64 + next() / 2, 64 + next() / 2);
        image::Rgba([x, y, 255, 255])
    });
    let path = rusty_test_support::temp::dir().join("rusty_1702_taa_bumps.png");
    img.save(&path).unwrap();
    let settings = TextureSettings {
        filter_mode: FilterMode::Bilinear,
        generate_mipmaps: false,
        ..TextureSettings::default()
    };
    rusty_asset::sidecar::set_texture(&path, settings).unwrap();
    path.to_string_lossy().into_owned()
}

fn sphere_scene(taa: bool) -> (Scene, u32) {
    let (mut scene, _) = taa_scene(taa, true);
    let sky = rusty_test_support::temp::dir().join("rusty_1702_taa_black_sky.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([0, 0, 0, 255]))
        .save(&sky)
        .unwrap();
    scene.skybox_path = sky.to_string_lossy().into_owned();
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Sphere", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, -3.5);
    let material = Some(MaterialComponent::new("m".to_string()));
    scene.world.set_material(id, material);
    let asset = MaterialAsset {
        metallic: 1.0,
        roughness: 0.08,
        normal_map: Some(bumpy_normal_map().into()),
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), asset);
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(1.0, 1.5, -1.0);
    let light = LightComponent {
        intensity: 20.0,
        range: 20.0,
        cast_shadows: false,
        mode: rusty_components::LightMode::Mixed,
        ..default_light()
    };
    scene.world.set_light(lamp, Some(light));
    (scene, id)
}

fn lumas(taa: bool) -> Option<Vec<Vec<f64>>> {
    let mut r = renderer()?;
    let (mut scene, id) = sphere_scene(taa);
    let mut v = view(&r);
    let mut out = Vec::new();
    for f in 0..WARM + MEASURED {
        let turn = Quat::from_rotation_y((f as f32 * STEP).to_radians());
        scene.world.transform_mut(id).unwrap().rotation = turn;
        let rgba = frame(&mut r, &mut v, &scene, &eye(0.0));
        let luma = rgba
            .chunks(4)
            .map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64);
        if f >= WARM {
            out.push(luma.collect());
        }
    }
    Some(out)
}

fn relative_flicker(frames: &[Vec<f64>]) -> f64 {
    let n = frames.len() as f64;
    let (mut flicker, mut signal) = (0.0, 0.0);
    for px in 0..frames[0].len() {
        let mean = frames.iter().map(|f| f[px]).sum::<f64>() / n;
        let steps = frames.windows(2).map(|w| (w[1][px] - w[0][px]).powi(2));
        flicker += steps.sum::<f64>() / (n - 1.0);
        signal += mean * mean;
    }
    flicker / signal.max(1e-9)
}

#[test]
fn gpu_a_turning_glossy_bumpy_sphere_shimmers_less_under_taa_than_fxaa() {
    let Some(fxaa) = lumas(false) else { return };
    let taa = lumas(true).expect("an adapter, as above");
    let (f, t) = (relative_flicker(&fxaa), relative_flicker(&taa));
    eprintln!(
        "[taa] relative flicker: FXAA {f:.5}, TAA {t:.5} ({:.1}x)",
        f / t
    );
    assert!(
        t * FACTOR < f,
        "TAA shimmers {t:.5}, not {FACTOR}x below FXAA's {f:.5}"
    );
}
