//! A still view's rough reflection holds still under TAA's sub-pixel jitter
//! (#2249): a satin bronze ceiling over a small, very bright bulb in a dark room,
//! seen from below. The resolve's blur radius is a field read from the trace's
//! spread pyramid around each pixel; around a cluster of hits as small as the
//! bulb's, a field that thresholds the hits' count reached them on some offsets
//! and not others, so a patch of the reflection blinked between dark and bright
//! with the camera still. Measured on the SSR-resolved image itself, before TAA
//! blends frames: what each offset hands TAA.

use glam::Vec3;

use super::super::fixture::{paint, place};
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, MaterialAsset, MaterialComponent, Ssr};
use rusty_core::color::f16_to_f32;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const W: u32 = 320;
const H: u32 = 240;
/// A tile's side, in pixels.
const TILE: u32 = 8;
/// Frames measured: one cycle of the jitter's vertical (base 3) part, and more,
/// so its horizontal (base 2) one turns over too.
const FRAMES: u32 = 36;

/// A box at `at`, `size` across, painted `surface`.
fn block(s: &mut Scene, name: &str, (at, size): (Vec3, Vec3), surface: ([f32; 3], f32, f32)) {
    let id = create_entity(s, name, Some(Primitive::Box));
    place(s, id, at, size);
    paint(s, id, name, surface);
}

/// The room: a satin bronze ceiling 3 m up, dark walls and floor, no ambient, the
/// bulb hanging 0.5 m under the ceiling on a cord; TAA on (for its jitter), SSR
/// on, nothing else.
fn room() -> Scene {
    let mut s = Scene::new();
    (s.skybox_path, s.ambient_intensity) = (String::new(), 0.0);
    let cam = s.add_entity("Camera".to_string());
    let camera = CameraComponent {
        taa_active: true,
        fxaa_active: false,
        ..Default::default()
    };
    s.world.set_camera(cam, Some(camera));
    let mut volume = default_post_processing();
    volume.ssr = Ssr {
        active: true,
        ..Default::default()
    };
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let bronze = ([0.8, 0.55, 0.35], 1.0, 0.35);
    block(
        &mut s,
        "Ceiling",
        (Vec3::new(0.0, 3.05, -4.0), Vec3::new(12.0, 0.1, 12.0)),
        bronze,
    );
    let shell = [
        (Vec3::new(0.0, -0.05, -4.0), Vec3::new(12.0, 0.1, 12.0)),
        (Vec3::new(0.0, 1.5, -10.0), Vec3::new(12.0, 3.0, 0.1)),
        (Vec3::new(-6.0, 1.5, -4.0), Vec3::new(0.1, 3.0, 12.0)),
        (Vec3::new(6.0, 1.5, -4.0), Vec3::new(0.1, 3.0, 12.0)),
        (Vec3::new(0.0, 1.5, 2.0), Vec3::new(12.0, 3.0, 0.1)),
        (Vec3::new(0.0, 2.8, -3.0), Vec3::new(0.01, 0.5, 0.01)),
    ];
    for (i, place) in shell.into_iter().enumerate() {
        block(&mut s, &format!("Shell{i}"), place, ([0.03; 3], 0.0, 1.0));
    }
    let bulb = create_entity(&mut s, "Bulb", Some(Primitive::Sphere));
    place(&mut s, bulb, Vec3::new(0.0, 2.55, -3.0), Vec3::splat(0.08));
    let glow = MaterialAsset {
        emissive: [40.0, 32.0, 22.0],
        ..MaterialAsset::default()
    };
    s.materials.insert("Bulb".into(), glow);
    let material = Some(MaterialComponent::new("Bulb".to_string()));
    s.world.set_material(bulb, material);
    s
}

/// Seated, looking up at the ceiling.
fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, 30.0)
}

/// The SSR-resolved HDR image per 8×8 tile, in 255ths of perceptual luma
/// (`l / (1 + l)`), so a tile's swing reads as a byte would before the tonemap.
fn tiles(r: &Renderer, v: &RenderView) -> Vec<f32> {
    let bytes = read_texture_bytes(&r.device, &r.queue, &v.post_fx.scene_hdr.texture, (W, H), 8);
    let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
    let luma = |x: u32, y: u32| {
        let i = ((y * W + x) * 8) as usize;
        let l = 0.25 * half(i) + 0.5 * half(i + 2) + 0.25 * half(i + 4);
        255.0 * l / (1.0 + l)
    };
    let tile = |tx: u32, ty: u32| {
        let px = (0..TILE * TILE).map(|k| luma(tx * TILE + k % TILE, ty * TILE + k / TILE));
        px.sum::<f32>() / (TILE * TILE) as f32
    };
    let (cols, rows) = (W / TILE, H / TILE);
    (0..cols * rows).map(|t| tile(t % cols, t / cols)).collect()
}

/// The largest step any tile takes between two frames of `frames`, and the tile.
fn swing(frames: &[Vec<f32>]) -> (f32, usize) {
    let steps = frames.windows(2).flat_map(|w| {
        let pairs = w[0].iter().zip(&w[1]).enumerate();
        pairs.map(|(t, (a, b))| ((a - b).abs(), t))
    });
    steps.fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a })
}

/// The largest step a tile of the SSR-resolved still view may take between two
/// jitter offsets, in 255ths of perceptual luma. Near the scene's own: with SSR
/// off the bulb's edge under the jitter steps 18 (lavapipe). Measured on
/// lavapipe: 46 before the field was made continuous (#2249), MEASURED after.
const STILL: f32 = 28.0;

#[test]
fn gpu_a_still_rough_reflection_holds_under_taa_jitter() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = room();
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let frames: Vec<Vec<f32>> = (0..FRAMES)
        .map(|_| {
            let target = v.color_target_view().unwrap();
            r.render(&mut v, &scene, &eye(), &target, false);
            tiles(&r, &v)
        })
        .collect();
    let (step, t) = swing(&frames);
    let series: Vec<String> = frames.iter().map(|f| format!("{:.0}", f[t])).collect();
    eprintln!(
        "[ssr] a still view's largest tile step under jitter: {step:.1} (tile {t}: {})",
        series.join(" ")
    );
    assert!(
        step <= STILL,
        "a still rough reflection blinks under the jitter: a tile steps {step:.1}"
    );
}
