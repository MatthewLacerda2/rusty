use glam::Vec3;

use crate::motion::gpu_fixture::{H, W};
use crate::postfx::effects::taa::gpu::fixture::{
    big_view, downsample, errors, frame, holds, renderer, settle,
};
use crate::postfx::effects::taa::gpu::fixture::{eye, taa_scene, tilted_cube};
use crate::postfx::effects::taa::TaaHistory;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_core::settings::render_scale::RenderScale;
use rusty_scene::{Camera, Scene};

pub(super) fn scaled(r: &Renderer, scale: f32) -> RenderView {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    v.set_render_scale(RenderScale::new(scale));
    v.resize(&r.device, W, H);
    v
}

pub(super) fn fresh_at(r: &mut Renderer, scale: f32, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut v = scaled(r, scale);
    frame(r, &mut v, scene, cam)
}

pub(super) fn upscales(v: &RenderView) -> bool {
    let h = v.post_fx.targets.get::<TaaHistory>();
    h.is_some_and(|h| h.upscale && h.size == (W, H))
}

pub(super) const SUPER: u32 = 8;
pub(super) const FRAMES: u32 = 64;

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
