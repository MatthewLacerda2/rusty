//! #1846 on lavapipe: a still frame converges exactly as URP's resolve does, its
//! two rules on or off, at the scene's size and upscaling: a noise wall behind a
//! cube, and a fence whose gaps, a third of a pixel, show the sky in some frames'
//! jitter and not in others' (no reveal: the test reads the 3×3's nearest surface).
//! And what the other tests compare a frame with one rule off by ([`run`]).

use glam::Vec3;

use super::super::params::Rules;
use super::super::TaaHistory;
use super::fixture::{cube, eye, frame, renderer, taa_scene, tilted_cube, view};
use super::wall::noisy_wall;
use crate::motion::gpu_fixture::{H, W};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_core::settings::render_scale::RenderScale;
use rusty_scene::{Camera, Scene};

/// Both rules off: URP's resolve.
pub(crate) const OFF: Rules = Rules {
    moving: false,
    reveal: false,
};
/// The reveal test off, the moving pixel's weight kept.
pub(super) const NO_REVEAL: Rules = Rules {
    moving: true,
    reveal: false,
};

/// The last of `n` frames of `scene` in `v`, frame `f` through `cam(f)`, the
/// resolve keeping only `rules` from the second frame (the first holds no history).
pub(crate) fn run(
    r: &mut Renderer,
    v: &mut RenderView,
    (scene, cam): (&Scene, &dyn Fn(u32) -> Camera),
    n: u32,
    rules: Rules,
) -> Vec<u8> {
    let mut last = frame(r, v, scene, &cam(0));
    keep(v, rules);
    for f in 1..n {
        last = frame(r, v, scene, &cam(f));
    }
    last
}

/// `v`'s resolve keeps only `rules` from its next frame on; TAA must have run.
pub(super) fn keep(v: &mut RenderView, rules: Rules) {
    let h = v.post_fx.targets.get_mut::<TaaHistory>().expect("TAA ran");
    h.rules = rules;
}

/// How many bytes of two frames differ.
pub(crate) fn differing(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(a, b)| a != b).count()
}

fn scaled(r: &Renderer, scale: f32) -> RenderView {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    v.set_render_scale(RenderScale::new(scale));
    v.resize(&r.device, W, H);
    v
}

/// A fence 5 m out of slats 0.46 m wide, 4 cm apart: a third of a pixel.
fn fence(scene: &mut Scene) {
    for i in -6..6 {
        let id = cube(scene, Vec3::new(i as f32 * 0.5 + 0.25, 0.0, -5.0));
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.scale, t.rotation) = (Vec3::new(0.46, 3.0, 0.1), glam::Quat::from_rotation_z(0.05));
    }
}

#[test]
fn gpu_a_still_frame_converges_exactly_as_urps_resolve() {
    let Some(mut r) = renderer() else { return };
    let (mut walled, _) = taa_scene(true, true);
    noisy_wall(&mut walled, -7.0);
    tilted_cube(&mut walled, Vec3::new(0.0, 0.0, -4.0));
    let (mut fenced, _) = taa_scene(true, true);
    fence(&mut fenced);
    let still = |_: u32| eye(0.0);
    for (scale, scene) in [1.0, 0.5]
        .into_iter()
        .flat_map(|s| [(s, &walled), (s, &fenced)])
    {
        let make = |r: &Renderer| match scale {
            1.0 => view(r),
            s => scaled(r, s),
        };
        let (mut a, mut b) = (make(&r), make(&r));
        let on = run(&mut r, &mut a, (scene, &still), 24, Rules::default());
        let off = run(&mut r, &mut b, (scene, &still), 24, OFF);
        let n = differing(&on, &off);
        assert_eq!(n, 0, "at render scale {scale}, {n} bytes differ from URP's");
    }
}
