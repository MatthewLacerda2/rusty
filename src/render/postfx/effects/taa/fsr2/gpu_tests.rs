//! FSR2 on lavapipe (#1848): what it shares with its other GPU tests, and the
//! test that justifies it, a thin wire at render scale 0.5 resolved nearer the
//! supersampled reference than TAAU's.

use glam::{Quat, Vec3, Vec4};

use super::Fsr2Targets;
use crate::core::settings::render_scale::RenderScale;
use crate::core::settings::upscaler::Upscaler;
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::postfx::effects::taa::gpu::fixture::{
    big_view, cube, downsample, errors, eye, frame, renderer, settle, taa_scene,
};
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::Scene;

/// The supersampled reference's factor, and the frames a still view settles.
pub(super) const SUPER: u32 = 8;
pub(super) const FRAMES: u32 = 64;

/// A fixture-size view whose scene renders at `scale`.
pub(super) fn scaled(r: &Renderer, scale: f32) -> RenderView {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    v.set_render_scale(RenderScale::new(scale));
    v.resize(&r.device, W, H);
    v
}

/// Whether FSR2 resolved `v`'s last frame.
pub(super) fn ran(v: &RenderView) -> bool {
    let f = v.post_fx.targets.get::<Fsr2Targets>();
    f.is_some_and(|f| f.output().is_some())
}

/// A wire `thick` metres across the fixture's view, 5 m out and slightly tilted.
pub(super) fn wire(scene: &mut Scene, thick: f32) {
    let id = cube(scene, Vec3::new(0.0, 0.0, -5.0));
    let t = &mut scene.world.transform_mut(id).unwrap();
    t.scale = Vec3::new(8.0, thick, thick);
    t.rotation = Quat::from_rotation_z(0.12);
}

/// `scene` settled through `upscaler` at `scale`: the frame's per-pixel error
/// against `reference`, and whether FSR2 ran.
fn settled(
    r: &mut Renderer,
    (upscaler, scale): (Upscaler, f32),
    scene: &Scene,
    reference: &[Vec4],
) -> (Vec<f32>, bool) {
    r.upscaler = upscaler;
    let mut v = scaled(r, scale);
    let f = settle(r, &mut v, scene, &eye(0.0), FRAMES);
    (errors(&f, reference), ran(&v))
}

/// The mean of `e` over `at`.
fn mean(e: &[f32], at: &[usize]) -> f32 {
    at.iter().map(|&i| e[i]).sum::<f32>() / at.len().max(1) as f32
}

#[test]
fn gpu_a_thin_wire_at_half_scale_is_nearer_native_under_fsr2_than_taau() {
    let Some(mut r) = renderer() else { return };
    let (mut plain, _) = taa_scene(false, false);
    wire(&mut plain, 0.12);
    let mut big = big_view(&r, SUPER);
    let reference = downsample(&frame(&mut r, &mut big, &plain, &eye(0.0)), SUPER);
    let (mut scene, _) = taa_scene(true, true);
    wire(&mut scene, 0.12);
    let (taau, taau_fsr2) = settled(&mut r, (Upscaler::Taa, 0.5), &scene, &reference);
    let (fsr2, fsr2_ran) = settled(&mut r, (Upscaler::Fsr2, 0.5), &scene, &reference);
    assert!(
        !taau_fsr2 && fsr2_ran,
        "each upscaler ran where it was chosen"
    );
    // The wire's pixels: where either frame is off the reference.
    let wire: Vec<usize> = (0..reference.len())
        .filter(|&i| taau[i].max(fsr2[i]) > 8.0)
        .collect();
    let all: Vec<usize> = (0..reference.len()).collect();
    let (t, f) = (mean(&taau, &wire), mean(&fsr2, &wire));
    let (ta, fa) = (mean(&taau, &all), mean(&fsr2, &all));
    eprintln!(
        "[fsr2] {} wire pixels off the 8x reference: TAAU {t:.2}, FSR2 {f:.2}; whole frame {ta:.3} vs {fa:.3}",
        wire.len()
    );
    assert!(wire.len() > 20, "the wire crosses the frame");
    assert!(f < t, "FSR2's wire: {f:.2} vs TAAU's {t:.2}");
    assert!(fa < ta, "FSR2's whole frame: {fa:.3} vs TAAU's {ta:.3}");
}
