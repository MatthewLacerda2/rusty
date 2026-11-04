//! Geometric specular anti-aliasing (#1702): a glossy, normal-mapped metal sphere
//! small on screen, turned a few degrees a frame under one lamp. Without the AA its
//! highlight breaks into single-pixel sparkles that jump from frame to frame; with
//! it, the lobe is widened by the spread of normals each pixel covers, and every
//! pixel's brightness changes far less from frame to frame.
//!
//! The measure is each pixel's variance in luma over the frames, relative to the
//! signal (`relative_temporal_variance`). Each frame is its own deterministic render (no temporal
//! filter), so the variance is the shimmer a moving camera would show. Without an
//! adapter the capture returns `Ok(false)` and the test skips.

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 96;
const FRAMES: u32 = 8;

/// A 64x64 tangent-space normal map of seeded noise, every texel tilted up to ~35°.
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

/// The sphere turned `degrees` about Y, smooth white metal with the bumpy normal map,
/// lit by one lamp over a black sky.
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
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

/// Every frame's luma, row-major; `None` without an adapter.
fn frames() -> Option<Vec<Vec<f64>>> {
    let map = bumpy_normal_map();
    // From +Z down -Z, far enough that the unit sphere spans about a third of the frame.
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

/// Each pixel's variance over `frames`, summed, over the sum of each pixel's mean
/// squared: the frame-to-frame flicker relative to the signal, so a highlight that
/// grows broader and brighter is not scored as more shimmer. Also the summed mean.
fn relative_temporal_variance(frames: &[Vec<f64>]) -> (f64, f64) {
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

/// Measured on lavapipe: 0.77 with the roughness left unwidened, 0.15 with the AA.
const BOUND: f64 = 0.35;

#[test]
fn a_turning_glossy_bumpy_sphere_shimmers_less_than_the_bound() {
    let Some(frames) = frames() else {
        return;
    };
    let (relative, total) = relative_temporal_variance(&frames);
    eprintln!("[specular_aa] relative temporal variance {relative:.3} (summed luma {total:.0})");
    assert!(
        total > 1000.0,
        "the highlight did not render (summed luma {total:.0})"
    );
    assert!(
        relative < BOUND,
        "the highlight shimmers: {relative:.3} >= {BOUND}"
    );
}
