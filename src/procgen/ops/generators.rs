//! src/procgen/ops/generators.rs — source ops (no inputs).
//!
//! Every generator paints a whole [`Image`] from its parameters alone, sampling the
//! **unit domain** `[0, 1)²` per pixel. They are seamless by construction: the
//! patterns are periodic over the domain (checker/brick/wave use integer cell counts;
//! noise/Voronoi wrap their lattice through [`super::super::hash`]), so a baked map
//! tiles without any seam-fix pass.

use super::super::hash;
use super::super::image_buf::{Image, Rgba};
use super::super::recipe::{GradientKind, NoiseKind, VoronoiOutput, WaveKind};

const GRAY_A: f32 = 1.0;

/// Pack a scalar `v` into an opaque grayscale pixel.
#[inline]
fn gray(v: f32) -> Rgba {
    [v, v, v, GRAY_A]
}

/// A flat color fill.
pub fn constant(resolution: u32, color: Rgba) -> Image {
    Image::filled(resolution, color)
}

/// Per-pixel uniform white noise, grayscale, seeded by the recipe seed.
pub fn white_noise(resolution: u32, seed: u64) -> Image {
    Image::fill_uv(resolution, |u, v| {
        let x = (u * resolution as f32) as i64;
        let y = (v * resolution as f32) as i64;
        gray(hash::unit2(x, y, 0, seed))
    })
}

/// Checkerboard: `tiles` squares across each axis; alternating `a`/`b`.
pub fn checker(resolution: u32, tiles: u32, a: Rgba, b: Rgba) -> Image {
    let t = tiles.max(1) as f32;
    Image::fill_uv(resolution, |u, v| {
        let cx = (u * t).floor() as i64;
        let cy = (v * t).floor() as i64;
        if (cx + cy).rem_euclid(2) == 0 {
            a
        } else {
            b
        }
    })
}

/// Linear (left→right) or radial (center→edge) grayscale gradient.
pub fn gradient(resolution: u32, kind: GradientKind) -> Image {
    Image::fill_uv(resolution, |u, v| match kind {
        GradientKind::Linear => gray(u),
        GradientKind::Radial => {
            let dx = u - 0.5;
            let dy = v - 0.5;
            // Normalize so the corner (the farthest point) reads ~1.0.
            let d = (dx * dx + dy * dy).sqrt() / std::f32::consts::FRAC_1_SQRT_2;
            gray(d.min(1.0))
        }
    })
}

/// Bands (parallel) or rings (concentric); `frequency` cycles across the domain.
/// The result is a `[0, 1]` sine band so it stays smooth and tiling.
pub fn wave(resolution: u32, kind: WaveKind, frequency: f32) -> Image {
    let tau = std::f32::consts::TAU;
    Image::fill_uv(resolution, |u, v| {
        let phase = match kind {
            WaveKind::Bands => u * frequency,
            WaveKind::Rings => {
                let dx = u - 0.5;
                let dy = v - 0.5;
                (dx * dx + dy * dy).sqrt() * frequency
            }
        };
        gray(0.5 + 0.5 * (phase * tau).sin())
    })
}

/// Running-bond brick: `cols`×`rows` bricks, every other row offset half a brick.
/// Mortar gaps read 0, brick faces read 1 (a mask).
pub fn brick(resolution: u32, rows: f32, cols: f32, mortar: f32) -> Image {
    let rows = rows.max(1.0);
    let cols = cols.max(1.0);
    Image::fill_uv(resolution, |u, v| {
        let ry = v * rows;
        let row = ry.floor() as i64;
        // Offset alternate rows by half a brick for the running bond.
        let offset = if row.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let cx = (u * cols + offset).fract();
        let cy = ry.fract();
        let in_mortar = cx < mortar || cy < mortar;
        gray(if in_mortar { 0.0 } else { 1.0 })
    })
}

/// Voronoi / Worley cellular pattern at `scale` cells across the domain. Either F1
/// distance (grayscale) or a flat random color per cell.
pub fn voronoi(resolution: u32, scale: f32, output: VoronoiOutput, seed: u64) -> Image {
    let cells = scale.max(1.0);
    Image::fill_uv(resolution, |u, v| {
        let px = u * cells;
        let py = v * cells;
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let mut best = f32::MAX;
        let mut best_cell = (gx, gy);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (gx + dx, gy + dy);
                // Jittered feature point inside the wrapped cell.
                let jx = hash::unit2(
                    cx.rem_euclid(cells as i64),
                    cy.rem_euclid(cells as i64),
                    1,
                    seed,
                );
                let jy = hash::unit2(
                    cx.rem_euclid(cells as i64),
                    cy.rem_euclid(cells as i64),
                    2,
                    seed,
                );
                let fx = cx as f32 + jx;
                let fy = cy as f32 + jy;
                let dist = (fx - px).hypot(fy - py);
                if dist < best {
                    best = dist;
                    best_cell = (cx.rem_euclid(cells as i64), cy.rem_euclid(cells as i64));
                }
            }
        }
        match output {
            VoronoiOutput::Distance => gray(best.min(1.0)),
            VoronoiOutput::Cells => {
                let r = hash::unit2(best_cell.0, best_cell.1, 3, seed);
                let g = hash::unit2(best_cell.0, best_cell.1, 4, seed);
                let b = hash::unit2(best_cell.0, best_cell.1, 5, seed);
                [r, g, b, 1.0]
            }
        }
    })
}

/// Perlin / fBM gradient noise, grayscale, mapped to `[0, 1]`. The lattice wraps at
/// `scale` so the result tiles.
pub fn noise(resolution: u32, kind: NoiseKind, scale: f32, octaves: u32, seed: u64) -> Image {
    let base = scale.max(1.0);
    Image::fill_uv(resolution, |u, v| {
        let n = match kind {
            NoiseKind::Perlin => perlin_tiling(u, v, base, seed),
            NoiseKind::Fbm => fbm(u, v, base, octaves.max(1), seed),
        };
        gray(n * 0.5 + 0.5)
    })
}

/// Sum several Perlin octaves at doubling frequency / halving amplitude.
fn fbm(u: f32, v: f32, base: f32, octaves: u32, seed: u64) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut freq = base;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += amp * perlin_tiling(u, v, freq, seed.wrapping_add(o as u64 * 0x9e37));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// A single octave of tiling gradient (Perlin) noise in `[-1, 1]`. The integer
/// lattice is taken modulo `period` so the field is periodic and tiles seamlessly.
fn perlin_tiling(u: f32, v: f32, period: f32, seed: u64) -> f32 {
    let p = period.max(1.0);
    let (x, y) = (u * p, v * p);
    let (x0, y0) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let per = p as i64;
    let corner = |cx: i64, cy: i64, lx: f32, ly: f32| {
        let (wx, wy) = (cx.rem_euclid(per), cy.rem_euclid(per));
        let ang = hash::unit2(wx, wy, 6, seed) * std::f32::consts::TAU;
        let (gx, gy) = (ang.cos(), ang.sin());
        gx * lx + gy * ly
    };
    let n00 = corner(x0, y0, fx, fy);
    let n10 = corner(x0 + 1, y0, fx - 1.0, fy);
    let n01 = corner(x0, y0 + 1, fx, fy - 1.0);
    let n11 = corner(x0 + 1, y0 + 1, fx - 1.0, fy - 1.0);
    let sx = smoothstep(fx);
    let sy = smoothstep(fy);
    let ix0 = lerp(n00, n10, sx);
    let ix1 = lerp(n01, n11, sx);
    lerp(ix0, ix1, sy)
}

#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
