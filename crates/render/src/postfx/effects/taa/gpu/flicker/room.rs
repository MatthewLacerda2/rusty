//! The flicker case's scene (`super`) and its measure: the resolve's own output
//! (its HDR history) per 8×8 tile, as perceptual luma (`y / (1 + y)`) in 255ths,
//! so a tile's swing reads as a byte would before the tonemap and the exposure.

use glam::Vec3;

use super::super::super::params::Rules;
use super::super::still::keep;
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, MaterialAsset, MaterialComponent, Ssr};
use rusty_core::color::f16_to_f32;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

pub(super) const W: u32 = 320;
pub(super) const H: u32 = 240;
/// A tile's side, in pixels.
const TILE: u32 = 8;
/// Frames before the measure: one cycle of the jitter's vertical (base 3) part.
pub(super) const WARM: u32 = 27;

/// A headless renderer of the case's size, or `None`.
pub(super) fn renderer() -> Option<Renderer> {
    crate::test_gpu::headless_or_skip(W, H)
}

/// Seated, looking up at the ceiling.
pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, 30.0)
}

fn paint(s: &mut Scene, id: u32, name: &str, asset: MaterialAsset) {
    s.materials.insert(name.into(), asset);
    s.world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
}

/// A box named `name` at `at`, sized `size`, of `asset`.
pub(super) fn block(
    s: &mut Scene,
    name: &str,
    (at, size): (Vec3, Vec3),
    asset: MaterialAsset,
) -> u32 {
    let id = create_entity(s, name, Some(Primitive::Box));
    let mut t = s.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (at, size);
    drop(t);
    paint(s, id, name, asset);
    id
}

/// A matte surface of `grey`.
pub(super) fn matte(grey: f32) -> MaterialAsset {
    let base_color = [grey; 3];
    MaterialAsset {
        base_color,
        roughness: 1.0,
        ..MaterialAsset::default()
    }
}

/// The room: a satin bronze ceiling 3 m up, dark walls and floor, no ambient,
/// the bulb hanging 0.5 m under the ceiling on a cord; TAA on, SSR on, nothing else.
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
    volume.ssr = Ssr {
        active: true,
        ..Default::default()
    };
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let bronze = MaterialAsset {
        base_color: [0.8, 0.55, 0.35],
        metallic: 1.0,
        roughness: 0.35,
        ..MaterialAsset::default()
    };
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
        block(&mut s, &format!("Shell{i}"), place, matte(0.03));
    }
    let bulb = create_entity(&mut s, "Bulb", Some(Primitive::Sphere));
    let mut t = s.world.transform_mut(bulb).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, 2.55, -3.0), Vec3::splat(0.08));
    drop(t);
    let glow = MaterialAsset {
        emissive: [40.0, 32.0, 22.0],
        ..MaterialAsset::default()
    };
    paint(&mut s, bulb, "Bulb", glow);
    s
}

/// The resolve's last output in `v`, per tile, in 255ths of perceptual luma.
pub(super) fn tiles(r: &Renderer, v: &RenderView) -> Vec<f32> {
    let texture = v.post_fx.taa_latest().expect("TAA ran");
    let bytes = readback::read_texture_bytes(&r.device, &r.queue, texture, (W, H), 8);
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

/// Each frame's tiles, `n` frames of `scene` after [`WARM`], frame `f` of it
/// changed by `step(f)` first, the resolve keeping `rules` from its second frame.
pub(super) fn run(
    r: &mut Renderer,
    scene: &mut Scene,
    (n, rules): (u32, Rules),
    step: &dyn Fn(&mut Scene, u32),
) -> Vec<Vec<f32>> {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let mut out = Vec::new();
    for f in 0..WARM + n {
        step(scene, f);
        let target = v.color_target_view().unwrap();
        r.render(&mut v, scene, &eye(), &target, false);
        if f == 0 {
            keep(&mut v, rules);
        }
        if f >= WARM {
            out.push(tiles(r, &v));
        }
    }
    out
}

/// The largest step any tile takes between two frames of `frames`.
pub(super) fn swing(frames: &[Vec<f32>]) -> f32 {
    let steps = frames
        .windows(2)
        .flat_map(|w| w[0].iter().zip(&w[1]).map(|(a, b)| (a - b).abs()));
    steps.fold(0.0, f32::max)
}
