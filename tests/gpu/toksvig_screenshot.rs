//! Toksvig specular AA (#1702): a glossy metal floor with a busy normal map, the
//! camera turning a little each frame. Far off, the map's detail averages away in
//! its mips; without Toksvig the highlight stays mirror-sharp there and sparkles
//! from frame to frame, with it the lobe widens by what the mips lost. Up close
//! (the base level) nothing changes. Run for an uncompressed map (the length read
//! from the sample) and a BC5 one (the importer's per-level table). No adapter: skip.

use glam::Vec3;
use rusty::components::{LightComponent, MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

use super::specular_aa_screenshot::relative_temporal_variance;

const W: u32 = 128;
const H: u32 = 96;
const FRAMES: u32 = 8;

/// A 128² tangent-space normal map of seeded noise, each texel a whole normal
/// tilted up to ~35°.
fn busy_normal_map(name: &str) -> std::path::PathBuf {
    let mut state = 0x2545_f491u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 24) as u8
    };
    let img = image::RgbaImage::from_fn(128, 128, |_, _| {
        let (x, y) = (64 + next() / 2, 64 + next() / 2);
        let (nx, ny) = (x as f32 / 127.5 - 1.0, y as f32 / 127.5 - 1.0);
        let nz = (1.0 - nx * nx - ny * ny).sqrt();
        image::Rgba([x, y, (nz * 127.5 + 127.5).round() as u8, 255])
    });
    let path = crate::temp::dir().join(name);
    img.save(&path).expect("write the normal map");
    path
}

/// The 15 m floor, metal of `roughness` with `map` tiled 8×, under one lamp of
/// `intensity`, black sky.
fn scene(map: &std::path::Path, roughness: f32, intensity: f32) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    let m = Some(MaterialComponent::new("m".to_string()));
    scene.world.set_material(id, m);
    scene.materials.insert(
        "m".to_string(),
        MaterialAsset {
            metallic: 1.0,
            roughness,
            tiling: [8.0, 8.0],
            normal_map: Some(map.to_string_lossy().into_owned().into()),
            ..MaterialAsset::default()
        },
    );
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 1.2, -4.0);
    let light = LightComponent {
        intensity,
        range: 30.0,
        cast_shadows: false,
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

/// Every frame's luma of `scene` from `eye` at `pitch`, turning 0.4° a frame;
/// `None` without an adapter.
fn frames(scene: &Scene, eye: Vec3, pitch: f32) -> Option<Vec<Vec<f64>>> {
    let mut host = CaptureHost::new();
    let mut out = Vec::new();
    for f in 0..FRAMES {
        let camera = Camera::new(eye, -90.0 + f as f32 * 0.4, pitch);
        let path = crate::temp::dir().join(format!("rusty_1702_floor_{}_{f}.png", eye.z));
        if !capture_into(&mut host, scene, &camera, &path, W, H).expect("capture") {
            eprintln!("[toksvig] no GPU/software adapter — skipping");
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

fn mean(frames: &[Vec<f64>]) -> f64 {
    let n = (frames.len() * frames[0].len()) as f64;
    frames.iter().flatten().sum::<f64>() / n
}

const FAR: (Vec3, f32) = (Vec3::new(0.0, 0.6, 6.5), -4.0);
const NEAR: (Vec3, f32) = (Vec3::new(0.0, 0.25, -2.5), -60.0);

// Measured on lavapipe; `main`'s numbers are this test's with both texture kernels
// forced to 0. Glossy (roughness 0.08), far: relative temporal variance 0.605 (RGBA8)
// and 0.663 (BC5) on `main`, 0.074 and 0.078 here (8.2x and 8.5x less). Its mean
// luma rises (3.2 to 23.3, 3.1 to 21.7): `main`'s sparkles clip at 255, so its mean
// understates the light; the floor only checks none is lost.
const GLOSSY_BOUND: f64 = 0.2;
const GLOSSY_MEAN_FLOOR: f64 = 3.0;

/// Each run: the map's name, whether it is BC5, and two means `main` shows. Near,
/// at the base level, the frames are byte-identical to `main`'s; at roughness 0.3
/// under a dim lamp, where `main`'s highlights do not clip (peak 175), widening a
/// normalized lobe moves light without making or losing much (3.35 and 3.07 here).
const RUNS: [(&str, bool, f64, f64); 2] = [
    ("rusty_1702_rgba.png", false, 27.14, 3.20),
    ("rusty_1702_bc5.png", true, 28.81, 3.00),
];

#[test]
fn a_far_busy_glossy_floor_shimmers_less_keeps_its_light_and_is_unchanged_up_close() {
    for (name, bc5, near_mean, unclipped_mean) in RUNS {
        let map = busy_normal_map(name);
        if bc5 {
            rusty::asset::texture_import::refresh(&map, "").expect("encode BC5");
        }
        let glossy = scene(&map, 0.08, 30.0);
        let Some(far) = frames(&glossy, FAR.0, FAR.1) else {
            return;
        };
        let near = frames(&glossy, NEAR.0, NEAR.1).unwrap();
        let dim = frames(&scene(&map, 0.3, 1.0), FAR.0, FAR.1).unwrap();
        let (variance, _) = relative_temporal_variance(&far);
        let (far, near, dim) = (mean(&far), mean(&near), mean(&dim));
        eprintln!("[toksvig] {name}: far variance {variance:.3}, means far {far:.2} near {near:.2} dim {dim:.2}");
        assert!(variance < GLOSSY_BOUND, "{name} shimmers: {variance:.3}");
        assert!(
            far > GLOSSY_MEAN_FLOOR,
            "{name} lost its far light: {far:.2}"
        );
        assert!(
            (near - near_mean).abs() < 0.5,
            "{name} changed up close: {near:.2}"
        );
        let ratio = dim / unclipped_mean;
        assert!(
            (0.9..1.15).contains(&ratio),
            "{name} moved the energy: {dim:.2}"
        );
    }
}
