//! Geometric specular anti-aliasing: a glossy, normal-mapped metal sphere, small on screen,
//! turned a few degrees a frame under one lamp. Without the AA its highlight breaks into
//! single-pixel sparkles; with it, frame-to-frame luma variance (relative to the signal) is far
//! lower.

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::edit::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 96;
const FRAMES: u32 = 8;

fn bumpy_normal_map() -> std::path::PathBuf {
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
    let path = crate::temp::dir().join("rusty_1702_bumps.png");
    img.save(&path).expect("write the normal map");
    path
}

fn scene(degrees: f32, normal_map: &std::path::Path) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Sphere", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().rotation = Quat::from_rotation_y(degrees.to_radians());
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("m".to_string())));
    scene.materials.insert(
        "m".to_string(),
        MaterialAsset {
            base_color: [1.0, 1.0, 1.0],
            metallic: 1.0,
            roughness: 0.08,
            normal_map: Some(normal_map.to_string_lossy().into_owned().into()),
            ..MaterialAsset::default()
        },
    );
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(1.0, 1.5, 2.5);
    let light = LightComponent {
        intensity: 20.0,
        range: 20.0,
        cast_shadows: false,
        mode: rusty::components::LightMode::Mixed,
        ..rusty::edit::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

fn frames() -> Option<Vec<Vec<f64>>> {
    let map = bumpy_normal_map();
    let camera = Camera::new(Vec3::new(0.0, 0.0, 3.5), -90.0, 0.0);
    let mut host = CaptureHost::new();
    let mut out = Vec::new();
    for f in 0..FRAMES {
        let path = crate::temp::dir().join(format!("rusty_1702_sphere_{f}.png"));
        let shot = capture_into(
            &mut host,
            &scene(f as f32 * 3.0, &map),
            &camera,
            &path,
            SIZE,
            SIZE,
        );
        if !shot.expect("capture") {
            eprintln!("[specular_aa] no GPU/software adapter — skipping");
            return None;
        }
        let img = image::open(&path).expect("png").to_rgb8();
        let luma = img
            .pixels()
            .map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64);
        out.push(luma.collect());
    }
    Some(out)
}

/// Summed per-pixel luma variance over summed squared means (so a broader, brighter
/// highlight is not more shimmer), and the summed mean.
pub(crate) fn relative_temporal_variance(frames: &[Vec<f64>]) -> (f64, f64) {
    let n = frames.len() as f64;
    let (mut variance, mut signal, mut total) = (0.0, 0.0, 0.0);
    for px in 0..frames[0].len() {
        let values: Vec<f64> = frames.iter().map(|f| f[px]).collect();
        let mean = values.iter().sum::<f64>() / n;
        variance += values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        signal += mean * mean;
        total += mean;
    }
    (variance / signal.max(1e-9), total)
}

/// lavapipe: 0.77 unwidened, 0.18 with the AA.
const BOUND: f64 = 0.35;
/// Peak-luma floor, so a `widen` that turned the metal matte fails. lavapipe: 239 with the AA, 156
/// matte.
const PEAK_FLOOR: f64 = 200.0;

fn mean_peak(frames: &[Vec<f64>]) -> f64 {
    let peaks = frames.iter().map(|f| f.iter().cloned().fold(0.0, f64::max));
    peaks.sum::<f64>() / frames.len() as f64
}

#[test]
fn a_turning_glossy_bumpy_sphere_shimmers_less_than_the_bound() {
    let Some(frames) = frames() else {
        return;
    };
    let (relative, total) = relative_temporal_variance(&frames);
    let peak = mean_peak(&frames);
    eprintln!("[specular_aa] relative temporal variance {relative:.3} (summed luma {total:.0}, peak {peak:.1})");
    assert!(
        relative < BOUND,
        "the highlight shimmers: {relative:.3} >= {BOUND}"
    );
    assert!(
        peak > PEAK_FLOOR,
        "the highlight went matte: peak {peak:.1}"
    );
}
