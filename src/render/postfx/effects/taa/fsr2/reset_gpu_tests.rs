//! Where FSR2 runs and when it starts over (#1848), on lavapipe: at native scale
//! too, never on a supersampled view (TAA's resolve keeps it), and a resize, a
//! render-scale change and a cut each give a frame exactly a new view's first.

use super::gpu_tests::{ran, scaled, wire};
use super::Fsr2Targets;
use crate::core::settings::render_scale::RenderScale;
use crate::core::settings::upscaler::Upscaler;
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::postfx::effects::taa::gpu::fixture::{eye, frame, renderer, taa_scene};
use crate::render::postfx::effects::taa::TaaHistory;
use crate::render::RenderView;
use crate::scene::Scene;

/// The fixture's scene with TAA on and a wire across it.
fn wired() -> Scene {
    let (mut scene, _) = taa_scene(true, true);
    wire(&mut scene, 0.12);
    scene
}

/// FSR2's frame index in `v`: 1 on the first frame after a reset.
fn index(v: &RenderView) -> u32 {
    v.post_fx
        .targets
        .get::<Fsr2Targets>()
        .map_or(0, |f| f.frame)
}

#[test]
fn gpu_fsr2_runs_at_native_scale_and_never_supersampled() {
    let Some(mut r) = renderer() else { return };
    r.upscaler = Upscaler::Fsr2;
    let scene = wired();
    let mut native = scaled(&r, 1.0);
    frame(&mut r, &mut native, &scene, &eye(0.0));
    assert!(
        ran(&native) && native.post_fx.taa_ran(),
        "FSR2 at 1: native AA"
    );
    let mut big = scaled(&r, 1.5);
    frame(&mut r, &mut big, &scene, &eye(0.0));
    assert!(!ran(&big), "FSR2 only upscales");
    let h = big.post_fx.targets.get::<TaaHistory>();
    assert!(h.is_some_and(|h| h.frames.is_some()) && big.post_fx.taa_ran());
    // Back to TAA: FSR2's targets go.
    r.upscaler = Upscaler::Taa;
    frame(&mut r, &mut native, &scene, &eye(0.0));
    assert!(native.post_fx.targets.get::<Fsr2Targets>().is_none());
    assert!(native.post_fx.taa_ran());
}

#[test]
fn gpu_a_resize_a_scale_change_and_a_cut_start_fsr2_over() {
    let Some(mut r) = renderer() else { return };
    r.upscaler = Upscaler::Fsr2;
    let scene = wired();
    let cam = eye(0.0);
    let first = |r: &mut _, scale| {
        let mut v = scaled(r, scale);
        frame(r, &mut v, &scene, &cam)
    };
    let fresh_half = first(&mut r, 0.5);
    let mut v = scaled(&r, 0.5);
    (0..6).for_each(|_| drop(frame(&mut r, &mut v, &scene, &cam)));
    assert_eq!(index(&v), 6, "six frames accumulated");
    // A render-scale change.
    v.set_render_scale(RenderScale::new(0.67));
    v.resize(&r.device, W, H);
    let after = frame(&mut r, &mut v, &scene, &cam);
    assert_eq!(index(&v), 1);
    assert!(
        after == first(&mut r, 0.67),
        "a scale change is a fresh view's first frame"
    );
    // A resize, and back.
    (0..3).for_each(|_| drop(frame(&mut r, &mut v, &scene, &cam)));
    v.set_render_scale(RenderScale::new(0.5));
    v.resize(&r.device, W, H);
    assert!(frame(&mut r, &mut v, &scene, &cam) == fresh_half);
    // A cut: another scene (another id, as Play, Stop or a camera switch are).
    (0..3).for_each(|_| drop(frame(&mut r, &mut v, &scene, &cam)));
    let other = wired();
    let after_cut = frame(&mut r, &mut v, &other, &cam);
    assert_eq!(index(&v), 1, "a cut resets FSR2's frame index");
    let mut fresh = scaled(&r, 0.5);
    assert!(
        after_cut == frame(&mut r, &mut fresh, &other, &cam),
        "no ghost of the cut"
    );
}
