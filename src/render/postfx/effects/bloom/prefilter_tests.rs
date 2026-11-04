//! The prefilter's coverage (#1700), adapter-free: a model of `postfx/bloom.wgsl`'s
//! 13-tap reduction (`prefilter_gpu_tests` checks the shader itself) taking a
//! scene with one lamp to the chain's first mip, as each divisor does: straight at
//! `÷1` and `÷2`, through the half-size image and the resample past it
//! ([`PREFILTER_DIVISOR`]). A lamp at every position inside a first-mip texel's
//! block reaches the chain, with close to the same energy, so a spark moving by
//! single pixels doesn't switch its glow on and off.

use super::PREFILTER_DIVISOR;

type Image = Vec<Vec<f32>>;

/// A bilinear sample of `img` at `(x, y)` (pixel `i` spanning `i..i + 1`), clamped
/// at the edges as the chain's sampler is.
fn bilinear(img: &Image, x: f32, y: f32) -> f32 {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let at = |i: f32, j: f32| {
        let row = &img[(j.max(0.0) as usize).min(img.len() - 1)];
        row[(i.max(0.0) as usize).min(row.len() - 1)]
    };
    at(x0, y0) * (1.0 - tx) * (1.0 - ty)
        + at(x0 + 1.0, y0) * tx * (1.0 - ty)
        + at(x0, y0 + 1.0) * (1.0 - tx) * ty
        + at(x0 + 1.0, y0 + 1.0) * tx * ty
}

/// `boxes` and `fs_prefilter` / `fs_resample`: `img` down to `n`² texels `ratio`
/// of its pixels wide, the five boxes Karis-weighted when `karis`.
fn reduce(img: &Image, ratio: f32, n: usize, karis: bool) -> Image {
    let boxes = [
        (0, 0, 0.5),
        (-1, -1, 0.125),
        (1, -1, 0.125),
        (-1, 1, 0.125),
        (1, 1, 0.125),
    ];
    let texel = |i: usize, j: usize| {
        let (cx, cy) = ((i as f32 + 0.5) * ratio, (j as f32 + 0.5) * ratio);
        let (mut sum, mut weights) = (0.0, 0.0);
        for (bx, by, share) in boxes {
            let corners = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
            let tap = |(x, y): (i32, i32)| {
                let (x, y) = ((bx + x) as f32 * ratio / 2.0, (by + y) as f32 * ratio / 2.0);
                bilinear(img, cx + x, cy + y)
            };
            let b = corners.map(tap).iter().sum::<f32>() / 4.0;
            let w = if karis { share / (1.0 + b) } else { share };
            (sum, weights) = (sum + b * w, weights + w);
        }
        sum / weights
    };
    (0..n)
        .map(|j| (0..n).map(|i| texel(i, j)).collect())
        .collect()
}

const SIDE: usize = 48;

/// What a lamp of `value` at `(x, y)` puts into a chain at `divisor`, summed over
/// the first mip, times `divisor²` over `value` (1: its exact share). `sparse`
/// takes the scene straight to the first mip at any divisor, as before #1700's review.
fn energy(divisor: u32, (x, y): (usize, usize), value: f32, sparse: bool) -> f32 {
    let mut scene = vec![vec![0.0; SIDE]; SIDE];
    scene[y][x] = value;
    let (d, karis) = (divisor as usize, value > 1.0);
    let first = if divisor <= PREFILTER_DIVISOR || sparse {
        reduce(&scene, d as f32, SIDE / d, karis)
    } else {
        let p = PREFILTER_DIVISOR as usize;
        let pre = reduce(&scene, p as f32, SIDE / p, karis);
        reduce(&pre, d as f32 / p as f32, SIDE / d, false)
    };
    let total: f32 = first.iter().flatten().sum();
    total * (d * d) as f32 / value
}

/// A lamp of `value` at every position within one first-mip texel's block, far
/// from the edges, and the least and most energy any reaches the chain with.
fn spread(divisor: u32, value: f32, sparse: bool) -> (Vec<f32>, f32, f32) {
    let d = divisor as usize;
    let at = |k: usize| (24 + k % d, 24 + k / d);
    let e: Vec<f32> = (0..d * d)
        .map(|k| energy(divisor, at(k), value, sparse))
        .collect();
    let (lo, hi) = e
        .iter()
        .fold((f32::MAX, 0.0f32), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    (e, lo, hi)
}

#[test]
fn every_scene_pixel_reaches_the_chain_at_every_divisor() {
    for divisor in 1..=crate::core::settings::quality::MAX_RESOLUTION_DIVISOR {
        // A dim lamp, where the Karis weights hardly move: ±10% of its exact share
        // (measured: ÷3 0.94..1.07, the rest exactly 1).
        let (_, lo, hi) = spread(divisor, 0.01, false);
        assert!(lo > 0.9 && hi < 1.1, "÷{divisor}: {lo:.3}..{hi:.3}");
        // A bright one, which they damp: about the same share at every position
        // (measured: ÷3 0.59..0.72, the rest one value).
        let (_, lo, hi) = spread(divisor, 40.0, false);
        assert!(
            lo > 0.0 && hi / lo < 1.25,
            "÷{divisor}, bright: {lo:.3}..{hi:.3}"
        );
    }
}

#[test]
fn thirteen_taps_straight_from_the_scene_drop_pixels_past_half() {
    // What the resample fixes: the 13 taps one ÷3 or ÷4 texel apart on the scene.
    for (divisor, dropped) in [(3, 4), (4, 8)] {
        let (e, ..) = spread(divisor, 0.01, true);
        let zero = e.iter().filter(|&&v| v < 1e-6).count();
        assert_eq!(zero, dropped, "÷{divisor}");
    }
}
