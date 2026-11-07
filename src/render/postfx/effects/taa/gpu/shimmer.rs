//! The symptom #1702 exists for, on lavapipe: a glossy, normal-mapped metal sphere
//! turning a few degrees a frame under a lamp (part 1's specular-AA scene, in a
//! view kept across frames). Its highlight sparkles from frame to frame; TAA's
//! per-pixel frame-to-frame flicker must sit well below FXAA's.

use glam::{Quat, Vec3};

use super::fixture::{eye, frame, renderer, taa_scene, view};
use crate::components::{LightComponent, MaterialAsset, MaterialComponent};
use crate::scene::authoring::{create_entity, default_light, Primitive};
use crate::scene::Scene;

/// Frames to settle, then frames measured.
const WARM: u32 = 8;
const MEASURED: u32 = 12;
/// TAA's relative flicker must be below FXAA's by this factor. Measured on
/// lavapipe: FXAA 0.0146, TAA 0.0068 (2.1x). What TAA keeps is its own jitter's:
/// a still sphere flickers 0 under FXAA and a little under TAA, since each
/// jittered frame still weighs 0.1 (URP's frame influence).
const FACTOR: f64 = 1.5;
/// Degrees the sphere turns a frame: slow enough that a pixel's change from one
/// frame to the next is the highlight jumping, not the sphere's own turn.
const STEP: f32 = 1.0;

/// A 64×64 tangent-space normal map of seeded noise.
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
    let path = crate::test_temp::dir().join("rusty_1702_taa_bumps.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// The fixture scene with the sphere and a lamp; returns it and the sphere.
fn sphere_scene(taa: bool) -> (Scene, u32) {
    let (mut scene, _) = taa_scene(taa, true);
    // A black sky, so the measure is the sphere's.
    let sky = crate::test_temp::dir().join("rusty_1702_taa_black_sky.png");
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
        ..default_light()
    };
    scene.world.set_light(lamp, Some(light));
    (scene, id)
}

/// Each measured frame's luma, the sphere turning `STEP` a frame; `None` without an adapter.
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

/// The flicker: each pixel's frame-to-frame change squared, averaged over the
/// frames and summed, over each pixel's mean squared, summed. Successive frames,
/// not the whole window, so the sphere's own slow turn (which TAA must keep) is
/// not counted as shimmer; a highlight jumping between pixels is.
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
