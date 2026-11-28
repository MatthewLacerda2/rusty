use glam::Vec3;

use super::bottles::{put, surface, H, W};
use crate::postfx::effects::taa::gpu::flicker::measure::{luma, tiles_of};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::SsrAlgorithm::{Approximation, PbrAccumulation};
use rusty_components::{CameraComponent, MaterialAsset, SsrAlgorithm};
use rusty_scene::create::{default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const WARM: u32 = 24;
const FRAMES: u32 = 24;
/// Measured on lavapipe: 0.10 (a history held at 0.9 gives about 0.1), and 0.33 with the
/// reveal's gap a fixed 2 % of the distance.
const STILLED: f32 = 0.2;
/// Measured on lavapipe: 22.
const BLINKING: usize = 10;

fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, -2.0)
}

/// A long satin floor seen from eye height at a grazing angle under TAA, glowing pillars
/// standing on it from 20 to 90 m away, each reflected below its foot.
fn floor(algorithm: SsrAlgorithm, m: MaterialAsset, pillar: MaterialAsset) -> Scene {
    let mut s = Scene::new();
    (s.skybox_path, s.ambient_intensity) = (String::new(), 0.3);
    let cam = s.add_entity("Camera".to_string());
    let camera = CameraComponent {
        taa_active: true,
        fxaa_active: false,
        ..Default::default()
    };
    s.world.set_camera(cam, Some(camera));
    let mut volume = default_post_processing();
    (volume.ssr.active, volume.ssr.min_smoothness) = (true, 0.3);
    volume.ssr.algorithm = algorithm;
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let place = (Vec3::new(0.0, -0.05, -100.0), Vec3::new(160.0, 0.1, 220.0));
    put(&mut s, "floor", Primitive::Box, place, m);
    for i in 0..8 {
        let d = 20.0 + 10.0 * i as f32;
        let x = ((i % 3) as f32 - 1.0) * 0.3 * d;
        let at = (Vec3::new(x, 3.0, -d), Vec3::new(0.02 * d, 6.0, 0.5));
        put(
            &mut s,
            &format!("pillar{i}"),
            Primitive::Box,
            at,
            pillar.clone(),
        );
    }
    s
}

fn glow(emissive: f32) -> MaterialAsset {
    MaterialAsset {
        emissive: [emissive; 3],
        ..surface(0.0, 0.0, 1.0)
    }
}

/// The tiles the floor covers whole: it drawn glowing, the pillars dark.
fn floor_tiles(r: &mut Renderer) -> Vec<bool> {
    let s = floor(Approximation, glow(50.0), glow(0.0));
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let target = v.color_target_view().unwrap();
    r.render(&mut v, &s, &eye(), &target, false);
    let tiles = tiles_of(r, &v.post_fx.scene_hdr.texture, luma);
    tiles.iter().map(|&l| l > 240.0).collect()
}

/// Each tile's largest step between still frames of the SSR-resolved image.
fn steps(r: &mut Renderer, algorithm: SsrAlgorithm) -> Vec<f32> {
    let s = floor(algorithm, surface(0.8, 1.0, 0.35), glow(8.0));
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let mut last: Option<Vec<f32>> = None;
    let mut most = vec![0.0f32; ((W / 8) * (H / 8)) as usize];
    for f in 0..WARM + FRAMES {
        let target = v.color_target_view().unwrap();
        r.render(&mut v, &s, &eye(), &target, false);
        let now = tiles_of(r, &v.post_fx.scene_hdr.texture, luma);
        if let Some(was) = last.filter(|_| f > WARM) {
            for (m, (a, b)) in most.iter_mut().zip(was.iter().zip(&now)) {
                *m = m.max((a - b).abs());
            }
        }
        last = Some(now);
    }
    most
}

#[test]
fn gpu_a_grazing_floor_keeps_its_accumulated_reflection_under_taa_jitter() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mask = floor_tiles(&mut r);
    let now = steps(&mut r, Approximation);
    let kept = steps(&mut r, PbrAccumulation);
    let blinking: Vec<_> = (0..now.len())
        .filter(|&t| mask[t] && now[t] > 2.0)
        .collect();
    let share = blinking.iter().map(|&t| kept[t] / now[t]).sum::<f32>() / blinking.len() as f32;
    let tiles = blinking.len();
    eprintln!(
        "[ssr] over {tiles} blinking floor tiles, accumulated step / this frame's: {share:.2}"
    );
    assert!(
        tiles >= BLINKING,
        "the far floor reflects the pillars: {tiles} tiles"
    );
    assert!(
        share <= STILLED,
        "the far floor's history is dropped under the jitter: steps at {share:.2} of the frame's own"
    );
}
