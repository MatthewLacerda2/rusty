use glam::Vec3;

use super::super::fixture::{eye, renderer, settle, taa_scene, tilted_cube, view};
use crate::motion::gpu_fixture::{H, W};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_core::settings::render_scale::RenderScale;
use rusty_scene::Scene;

pub(super) fn sharpen_scene(strength: f32) -> (Scene, u32) {
    let (mut scene, cam) = taa_scene(true, true);
    tilted_cube(&mut scene, Vec3::new(0.0, 0.0, -4.0));
    let mut c = scene.world.camera_mut(cam).unwrap();
    c.taa_contrast_adaptive_sharpening = strength;
    drop(c);
    (scene, cam)
}

fn total_variation(frame: &[u8]) -> u64 {
    let luma = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        frame[i] as i32 + frame[i + 1] as i32 * 2 + frame[i + 2] as i32
    };
    let mut tv = 0u64;
    for y in 0..H {
        for x in 1..W {
            tv += luma(x, y).abs_diff(luma(x - 1, y)) as u64;
        }
    }
    tv
}

fn sharpens(view: &RenderView) -> bool {
    let targets = &view.post_fx.targets;
    targets
        .get::<super::super::super::sharpen::SharpenTargets>()
        .is_some()
}

#[test]
fn gpu_sharpen_at_zero_records_no_pass() {
    let Some(mut r) = renderer() else { return };
    let (zero_scene, _) = sharpen_scene(0.0);
    let (on_scene, _) = sharpen_scene(0.9);
    let (mut a, mut b) = (view(&r), view(&r));
    let zero = settle(&mut r, &mut a, &zero_scene, &eye(0.0), 4);
    assert!(!sharpens(&a), "strength 0 recorded the sharpen");
    let on = settle(&mut r, &mut b, &on_scene, &eye(0.0), 4);
    assert!(sharpens(&b), "strength 0.9 did not sharpen");
    assert_ne!(zero, on, "the sharpen changed nothing");
}

#[test]
fn gpu_sharpen_raises_edge_contrast_without_ringing() {
    let Some(mut r) = renderer() else { return };
    let (off_scene, _) = sharpen_scene(0.0);
    let (on_scene, _) = sharpen_scene(0.9);
    let (mut a, mut b) = (view(&r), view(&r));
    let off = settle(&mut r, &mut a, &off_scene, &eye(0.0), 24);
    let on = settle(&mut r, &mut b, &on_scene, &eye(0.0), 24);

    let (tv_off, tv_on) = (total_variation(&off), total_variation(&on));
    assert!(
        tv_on > tv_off,
        "sharpening should raise edge contrast: {tv_on} vs {tv_off}"
    );

    let mut flat = 0;
    for y in 1..H - 1 {
        for x in 1..W - 1 {
            for c in 0..3 {
                let at = |x: u32, y: u32| off[((y * W + x) * 4) as usize + c];
                let cross = [
                    at(x, y),
                    at(x - 1, y),
                    at(x + 1, y),
                    at(x, y - 1),
                    at(x, y + 1),
                ];
                let (lo, hi) = (cross.iter().min().unwrap(), cross.iter().max().unwrap());
                if hi - lo > 1 {
                    continue;
                }
                let d = on[((y * W + x) * 4) as usize + c].abs_diff(at(x, y));
                assert!(d <= 3, "flat ({x},{y}) channel {c} rang by {d}");
                flat += 1;
            }
        }
    }
    assert!(flat > 100, "too few flat pixels to judge: {flat}");
}

fn scaled(r: &Renderer, scale: f32) -> RenderView {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    v.set_render_scale(RenderScale::new(scale));
    v.resize(&r.device, W, H);
    v
}

#[test]
fn gpu_sharpen_runs_at_the_output_size_under_taau() {
    let Some(mut r) = renderer() else { return };
    for scale in [0.5, 0.33] {
        let (off_scene, _) = sharpen_scene(0.0);
        let (on_scene, _) = sharpen_scene(0.9);
        let (mut a, mut b) = (scaled(&r, scale), scaled(&r, scale));
        let off = settle(&mut r, &mut a, &off_scene, &eye(0.0), 24);
        let on = settle(&mut r, &mut b, &on_scene, &eye(0.0), 24);
        assert!(
            a.post_fx.taa_upscaled(),
            "scale {scale} should upscale through TAA"
        );
        let changed = off.iter().zip(&on).filter(|(a, b)| a != b).count();
        assert!(changed > 0, "sharpen did nothing at render scale {scale}");
        let dark = on
            .chunks(4)
            .filter(|px| px[0] as u32 + px[1] as u32 + px[2] as u32 == 0)
            .count();
        assert_eq!(
            dark, 0,
            "a size mismatch left black pixels at render scale {scale}"
        );
    }
}
