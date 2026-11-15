//! A still view's rough reflection holds still under TAA's sub-pixel jitter
//! (#2249, #2228): a satin bronze ceiling over a small, very bright bulb on a 1 cm
//! cord in a dark room, seen from below. The cord is under a pixel wide: on some
//! of TAA's offsets its reflection lands in one column, on others the next or
//! none. A radius field that took a plain mean let the cord's dark hits set the
//! bulb's blur, and the bulb's reflection blinked. Measured on the SSR-resolved
//! image (what each offset hands TAA) and after TAA's resolve.

use glam::Vec3;

use super::super::fixture::{paint, place};
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, MaterialAsset, MaterialComponent};
use rusty_core::color::f16_to_f32;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

pub(super) const W: u32 = 320;
pub(super) const H: u32 = 240;
/// A tile's side, in pixels.
pub(super) const TILE: u32 = 8;
/// Frames before the measure, so TAA's history has settled (its resolve keeps
/// about a tenth of each new frame).
const WARM: u32 = 27;
/// Frames measured: offsets 28 to 63 of the jitter's 1024 (Halton 2, 3), long
/// enough for a swing every two or three frames to show many times.
const FRAMES: u32 = 36;

/// A box at `at`, `size` across, painted `surface`.
fn block(s: &mut Scene, name: &str, (at, size): (Vec3, Vec3), surface: ([f32; 3], f32, f32)) {
    let id = create_entity(s, name, Some(Primitive::Box));
    place(s, id, at, size);
    paint(s, id, name, surface);
}

/// The room: a satin bronze ceiling 3 m up (the first box), dark walls and floor,
/// no ambient, the bulb hanging 0.5 m under the ceiling on a cord; TAA on (for its
/// jitter), SSR on, nothing else.
pub(super) fn room() -> Scene {
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
    volume.ssr.active = true;
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let shell = [
        (Vec3::new(0.0, 3.05, -4.0), Vec3::new(12.0, 0.1, 12.0)),
        (Vec3::new(0.0, -0.05, -4.0), Vec3::new(12.0, 0.1, 12.0)),
        (Vec3::new(0.0, 1.5, -10.0), Vec3::new(12.0, 3.0, 0.1)),
        (Vec3::new(-6.0, 1.5, -4.0), Vec3::new(0.1, 3.0, 12.0)),
        (Vec3::new(6.0, 1.5, -4.0), Vec3::new(0.1, 3.0, 12.0)),
        (Vec3::new(0.0, 1.5, 2.0), Vec3::new(12.0, 3.0, 0.1)),
        (Vec3::new(0.0, 2.8, -3.0), Vec3::new(0.01, 0.5, 0.01)),
    ];
    for (i, place) in shell.into_iter().enumerate() {
        let bronze = ([0.8, 0.55, 0.35], 1.0, 0.35);
        let surface = if i == 0 {
            bronze
        } else {
            ([0.03; 3], 0.0, 1.0)
        };
        block(&mut s, &format!("Shell{i}"), place, surface);
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

/// `texture` (an `Rgba16Float` image of the view's size) per 8×8 tile, in 255ths
/// of perceptual luma (`l / (1 + l)`), so a tile's swing reads as a byte would
/// before the tonemap.
fn tiles(r: &Renderer, texture: &wgpu::Texture) -> Vec<f32> {
    let bytes = read_texture_bytes(&r.device, &r.queue, texture, (W, H), 8);
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

/// The largest step any tile takes between two frames of `frames`.
fn swing(frames: &[Vec<f32>]) -> f32 {
    let steps = frames
        .windows(2)
        .flat_map(|w| w[0].iter().zip(&w[1]).map(|(a, b)| (a - b).abs()));
    steps.fold(0.0, f32::max)
}

/// The largest step a tile of the SSR-resolved still view may take between two
/// jitter offsets, in 255ths of perceptual luma: near the trace's own, which
/// without the resolve's blur steps 24 (the bulb's sharp reflection crossing a
/// tile's edge). Measured: 44.5 with a plain mean radius (lavapipe and an RTX
/// 2060), 20.8 with the field weighted by brightness (RTX 2060; CI's log
/// prints lavapipe's).
const STILL: f32 = 30.0;

/// The same after TAA's resolve, URP's clamp (#2228): what the player sees, a
/// patch swinging as a whole passing the clamp. Measured: 26.2 with a plain mean
/// radius (lavapipe and an RTX 2060), 7.5 weighted by brightness (RTX 2060).
const TAA_STILL: f32 = 14.0;

#[test]
fn gpu_a_still_rough_reflection_holds_under_taa_jitter() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = room();
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    // Seated, looking up at the ceiling.
    let eye = Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, 30.0);
    let (mut ssr, mut taa) = (Vec::new(), Vec::new());
    for f in 0..WARM + FRAMES {
        let target = v.color_target_view().unwrap();
        r.render(&mut v, &scene, &eye, &target, false);
        if f >= WARM {
            ssr.push(tiles(&r, &v.post_fx.scene_hdr.texture));
            taa.push(tiles(&r, v.post_fx.taa_latest().expect("TAA ran")));
        }
    }
    let (ssr, taa) = (swing(&ssr), swing(&taa));
    eprintln!("[ssr] a still view's largest tile step under jitter: {ssr:.1} resolved, {taa:.1} after TAA");
    assert!(
        ssr <= STILL,
        "a still rough reflection blinks under the jitter: a tile steps {ssr:.1}"
    );
    assert!(
        taa <= TAA_STILL,
        "a still view blinks under TAA: a tile steps {taa:.1}"
    );
}
