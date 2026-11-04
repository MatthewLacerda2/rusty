//! TAAU on lavapipe (#1702, part 3): a still frame at a render scale of 0.67
//! settles nearer the supersampled reference than the bilinear upscale, a thin wire
//! at 0.5 stays continuous, and the path is taken only when it should be. Ghosts
//! and cuts: `ghost_gpu_tests`.

use glam::Vec3;

use crate::core::settings::render_scale::RenderScale;
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::postfx::effects::taa::gpu_fixture::{
    big_view, downsample, errors, frame, holds, renderer, settle,
};
use crate::render::postfx::effects::taa::gpu_fixture::{cube, eye, taa_scene, tilted_cube};
use crate::render::postfx::effects::taa::TaaHistory;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

/// A fixture-size view whose scene renders at `scale`.
pub(super) fn scaled(r: &Renderer, scale: f32) -> RenderView {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    v.set_render_scale(RenderScale::new(scale));
    v.resize(&r.device, W, H);
    v
}

/// One frame of `scene` in a fresh fixture-size view at `scale`.
pub(super) fn fresh_at(r: &mut Renderer, scale: f32, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut v = scaled(r, scale);
    frame(r, &mut v, scene, cam)
}

/// Whether `v` upscales through TAA (its history is at the output's size).
pub(super) fn upscales(v: &RenderView) -> bool {
    let h = v.post_fx.targets.get::<TaaHistory>();
    h.is_some_and(|h| h.upscale && h.size == (W, H))
}

const SUPER: u32 = 8;
const FRAMES: u32 = 64;
/// The wire's thickness: half a scene pixel at 0.5, a quarter of one at native.
const WIRE: f32 = 0.12;

#[test]
fn gpu_a_still_frame_at_two_thirds_settles_nearer_native_than_bilinear() {
    let Some(mut r) = renderer() else { return };
    let (cam, at) = (eye(0.0), Vec3::new(0.0, 0.0, -4.0));
    let (mut plain, _) = taa_scene(false, false);
    tilted_cube(&mut plain, at);
    let mut big = big_view(&r, SUPER);
    let reference = downsample(&frame(&mut r, &mut big, &plain, &cam), SUPER);
    let (mut fxaa, _) = taa_scene(false, true);
    tilted_cube(&mut fxaa, at);
    let mut v = scaled(&r, 0.67);
    let bilinear = errors(&frame(&mut r, &mut v, &fxaa, &cam), &reference);
    assert!(!holds(&v), "TAA off: no history, the bilinear upscale");
    let (mut scene, _) = taa_scene(true, true);
    tilted_cube(&mut scene, at);
    let mut v = scaled(&r, 0.67);
    let taau = errors(&settle(&mut r, &mut v, &scene, &cam, FRAMES), &reference);
    assert!(upscales(&v), "TAA at 0.67 upscales at the output's size");
    // The edge pixels: where the bilinear frame is off the reference.
    let edges: Vec<usize> = (0..bilinear.len())
        .filter(|&i| bilinear[i] > 16.0)
        .collect();
    let mean = |e: &[f32]| edges.iter().map(|&i| e[i]).sum::<f32>() / edges.len() as f32;
    let (b, t) = (mean(&bilinear), mean(&taau));
    let all = |e: &[f32]| e.iter().sum::<f32>() / e.len() as f32;
    eprintln!(
        "[taau] {} edge pixels off the 8x reference: bilinear {b:.1}, TAAU {t:.1}; whole frame {:.2} vs {:.2}",
        edges.len(),
        all(&bilinear),
        all(&taau)
    );
    assert!(edges.len() > 20, "the cube's edges cross the frame");
    assert!(t < 0.6 * b, "TAAU's edges: {t:.1} vs bilinear's {b:.1}");
    assert!(all(&taau) < all(&bilinear), "TAAU's whole frame is nearer");
}

/// The columns a wire across the frame covers, and how much the dimmest one of
/// them shows it: each column's largest difference from the empty sky.
fn faintest_column(wire: &[u8], sky: &[u8]) -> u8 {
    let px = |f: &[u8], x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        [f[i], f[i + 1], f[i + 2]]
    };
    (8..W - 8)
        .map(|x| {
            (0..H)
                .map(|y| {
                    let (a, b) = (px(wire, x, y), px(sky, x, y));
                    (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap()
                })
                .max()
                .unwrap()
        })
        .min()
        .unwrap()
}

#[test]
fn gpu_a_thin_wire_at_half_scale_stays_continuous() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let wire_in = |scene: &mut Scene| {
        let id = cube(scene, Vec3::new(0.0, 0.0, -5.0));
        let t = &mut scene.world.transform_mut(id).unwrap();
        t.scale = Vec3::new(8.0, WIRE, WIRE);
        t.rotation = glam::Quat::from_rotation_z(0.12);
    };
    let (sky_scene, _) = taa_scene(false, false);
    let sky = fresh_at(&mut r, 0.5, &sky_scene, &cam);
    let (mut plain, _) = taa_scene(false, false);
    wire_in(&mut plain);
    let bilinear = faintest_column(&fresh_at(&mut r, 0.5, &plain, &cam), &sky);
    let (mut scene, _) = taa_scene(true, true);
    wire_in(&mut scene);
    let mut v = scaled(&r, 0.5);
    let taau = faintest_column(&settle(&mut r, &mut v, &scene, &cam, FRAMES), &sky);
    let mut big = big_view(&r, SUPER);
    let reference = downsample(&frame(&mut r, &mut big, &plain, &cam), SUPER);
    let bytes: Vec<u8> = reference
        .iter()
        .flat_map(|p| [p.x as u8, p.y as u8, p.z as u8, 255])
        .collect();
    let native = faintest_column(&bytes, &fresh_at(&mut r, 1.0, &sky_scene, &cam));
    eprintln!("[taau] the wire's faintest column at 0.5: bilinear {bilinear}, TAAU {taau}; 8x native {native}");
    // Lavapipe: bilinear 0 (a gap), TAAU 8, the reference 16.
    assert!(native >= 12, "the wire shows in the reference: {native}");
    assert!(
        4 * bilinear < native,
        "the wire must break up bilinearly: {bilinear}"
    );
    assert!(
        3 * taau >= native,
        "the wire has a gap under TAAU: {taau} of {native}"
    );
}
