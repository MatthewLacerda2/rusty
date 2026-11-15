//! How a flicker run (`super`) is measured: the resolve's own output (its HDR
//! history) per 8×8 tile, as perceptual luma (`y / (1 + y)`) in 255ths, so a
//! tile's swing reads as a byte would before the tonemap and the exposure, or by
//! any measure of its YCoCg ([`run_by`]).

use super::room::{eye, H, W, WARM};
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_core::color::f16_to_f32;
use rusty_core::settings::render_scale::RenderScale;
use rusty_scene::Scene;

/// A tile's side, in pixels.
const TILE: u32 = 8;

/// A pixel's perceptual luma in 255ths, from its YCoCg: [`run`]'s measure.
pub(super) fn luma(ycocg: [f32; 3]) -> f32 {
    255.0 * ycocg[0].max(0.0) / (1.0 + ycocg[0].max(0.0))
}

/// The resolve's last output in `v`, per tile, by `measure` of each pixel's YCoCg.
pub(super) fn tiles(r: &Renderer, v: &RenderView, measure: fn([f32; 3]) -> f32) -> Vec<f32> {
    let texture = v.post_fx.taa_latest().expect("TAA ran");
    let bytes = readback::read_texture_bytes(&r.device, &r.queue, texture, (W, H), 8);
    let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
    let pixel = |x: u32, y: u32| {
        let i = ((y * W + x) * 8) as usize;
        let (r, g, b) = (half(i), half(i + 2), half(i + 4));
        let y = 0.25 * r + 0.5 * g + 0.25 * b;
        measure([y, 0.5 * (r - b), 0.5 * g - 0.25 * (r + b)])
    };
    let tile = |tx: u32, ty: u32| {
        let px = (0..TILE * TILE).map(|k| pixel(tx * TILE + k % TILE, ty * TILE + k / TILE));
        px.sum::<f32>() / (TILE * TILE) as f32
    };
    let (cols, rows) = (W / TILE, H / TILE);
    (0..cols * rows).map(|t| tile(t % cols, t / cols)).collect()
}

/// Each frame's tiles of [`luma`], `n` frames of `scene` after [`WARM`], frame `f`
/// of it changed by `step(f)` first, at render `scale` (below 1, TAA upscales).
pub(super) fn run(
    r: &mut Renderer,
    scene: &mut Scene,
    frames: (u32, f32),
    step: &dyn Fn(&mut Scene, u32),
) -> Vec<Vec<f32>> {
    run_by(r, scene, frames, step, luma)
}

/// [`run`], each tile by `measure` of its pixels' YCoCg.
pub(super) fn run_by(
    r: &mut Renderer,
    scene: &mut Scene,
    (n, scale): (u32, f32),
    step: &dyn Fn(&mut Scene, u32),
    measure: fn([f32; 3]) -> f32,
) -> Vec<Vec<f32>> {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    if scale != 1.0 {
        v.set_render_scale(RenderScale::new(scale));
        v.resize(&r.device, W, H);
    }
    let mut out = Vec::new();
    for f in 0..WARM + n {
        step(scene, f);
        let target = v.color_target_view().unwrap();
        r.render(&mut v, scene, &eye(), &target, false);
        if f >= WARM {
            out.push(tiles(r, &v, measure));
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
