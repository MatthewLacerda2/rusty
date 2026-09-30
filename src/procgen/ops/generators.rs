//! src/procgen/ops/generators.rs — source ops (no inputs).
//!
//! Every generator is a pure **sampler** `(u, v) -> Rgba` over the unit domain
//! `[0, 1)²`; [`sampler`] builds one from an [`OpKind`] and the runner paints it per
//! pixel. They are seamless by construction because every periodic param is a
//! **count of periods per tile** ([`count`]): noise/Voronoi wrap an integer lattice
//! through [`super::super::hash`], and wave/brick/checker repeat a whole number of
//! times (#392). Two generators are the documented exceptions: `gradient linear` is a
//! one-way ramp (use `linear_tiling` for a seamless one), and the radial shapes
//! (`gradient radial`, `wave rings`) meet the edge mirror-symmetrically — continuous
//! across the seam, not periodic beyond it.

use super::super::hash;
use super::super::image_buf::Rgba;
use super::super::recipe::{GradientKind, NoiseKind, OpKind, VoronoiOutput, WaveKind};

/// A generator's per-point function over the unit domain.
pub type Sampler = Box<dyn Fn(f32, f32) -> Rgba>;

/// Pack a scalar `v` into an opaque grayscale pixel.
#[inline]
fn gray(v: f32) -> Rgba {
    [v, v, v, 1.0]
}

/// A periodic param as a whole number of periods per tile: rounded, at least 1. A
/// fractional count cannot close at the wrap, so it is never honoured.
#[inline]
pub fn count(x: f32) -> f32 {
    x.round().max(1.0)
}

/// Build the sampler for a generator op, or `None` if `op` takes inputs.
/// `resolution` only matters to `white_noise`, whose grain is one pixel.
pub fn sampler(op: &OpKind, resolution: u32, seed: u64) -> Option<Sampler> {
    Some(match *op {
        OpKind::Constant { color } => Box::new(move |_, _| color),
        OpKind::Noise {
            kind,
            scale,
            octaves,
        } => noise(kind, scale, octaves, seed),
        OpKind::Voronoi { scale, output } => voronoi(scale, output, seed),
        OpKind::Gradient { kind } => gradient(kind),
        OpKind::Wave { kind, frequency } => wave(kind, frequency),
        OpKind::Brick { rows, cols, mortar } => brick(rows, cols, mortar),
        OpKind::Checker {
            tiles,
            color_a,
            color_b,
        } => checker(tiles, color_a, color_b),
        OpKind::WhiteNoise => white_noise(resolution, seed),
        _ => return None,
    })
}

/// Per-pixel uniform white noise, grayscale. The pixel lattice wraps at `resolution`.
fn white_noise(resolution: u32, seed: u64) -> Sampler {
    let r = resolution.max(1) as i64;
    let res = resolution as f32;
    Box::new(move |u, v| {
        let x = ((u * res).floor() as i64).rem_euclid(r);
        let y = ((v * res).floor() as i64).rem_euclid(r);
        gray(hash::unit2(x, y, 0, seed))
    })
}

/// Checkerboard: `tiles` squares across each axis; alternating `a`/`b`. `tiles`
/// rounds **up to even**: an odd count would put two same-colour squares side by
/// side at the wrap.
fn checker(tiles: u32, a: Rgba, b: Rgba) -> Sampler {
    let t = tiles.max(1).next_multiple_of(2) as f32;
    Box::new(move |u, v| {
        let cx = (u * t).floor() as i64;
        let cy = (v * t).floor() as i64;
        if (cx + cy).rem_euclid(2) == 0 {
            a
        } else {
            b
        }
    })
}

/// Linear ramp (left→right, the one non-tiling generator), its seamless 0→1→0
/// triangle twin, or a radial (center→edge) ramp.
fn gradient(kind: GradientKind) -> Sampler {
    Box::new(move |u, v| match kind {
        GradientKind::Linear => gray(u),
        GradientKind::LinearTiling => gray(1.0 - (2.0 * u.rem_euclid(1.0) - 1.0).abs()),
        GradientKind::Radial => {
            let (dx, dy) = (u - 0.5, v - 0.5);
            // Normalize so the corner (the farthest point) reads ~1.0.
            let d = (dx * dx + dy * dy).sqrt() / std::f32::consts::FRAC_1_SQRT_2;
            gray(d.min(1.0))
        }
    })
}

/// Bands (parallel) or rings (concentric): a `[0, 1]` sine with `frequency` whole
/// cycles across the domain.
fn wave(kind: WaveKind, frequency: f32) -> Sampler {
    let f = count(frequency);
    Box::new(move |u, v| {
        let phase = match kind {
            WaveKind::Bands => u * f,
            WaveKind::Rings => (u - 0.5).hypot(v - 0.5) * f,
        };
        gray(0.5 + 0.5 * (phase * std::f32::consts::TAU).sin())
    })
}

/// Running-bond brick: `cols`×`rows` bricks, every other row offset half a brick.
/// `rows` rounds to an **even** count so the alternation closes at the wrap. Mortar
/// gaps read 0, brick faces read 1 (a mask).
fn brick(rows: f32, cols: f32, mortar: f32) -> Sampler {
    let rows = count(rows / 2.0) * 2.0;
    let cols = count(cols);
    Box::new(move |u, v| {
        let ry = v * rows;
        let offset = if (ry.floor() as i64).rem_euclid(2) == 0 {
            0.0
        } else {
            0.5
        };
        let cx = (u * cols + offset).rem_euclid(1.0);
        let cy = ry.rem_euclid(1.0);
        gray(if cx < mortar || cy < mortar { 0.0 } else { 1.0 })
    })
}

/// Voronoi / Worley cellular pattern with `scale` cells across the domain. Either F1
/// distance (grayscale) or a flat random color per cell.
fn voronoi(scale: f32, output: VoronoiOutput, seed: u64) -> Sampler {
    let cells = count(scale);
    let per = cells as i64;
    Box::new(move |u, v| {
        let (px, py) = (u * cells, v * cells);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let (mut best, mut best_cell) = (f32::MAX, (0, 0));
        for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
            let (cx, cy) = (gx + dx, gy + dy);
            let cell = (cx.rem_euclid(per), cy.rem_euclid(per));
            // Jittered feature point inside the wrapped cell.
            let fx = cx as f32 + hash::unit2(cell.0, cell.1, 1, seed);
            let fy = cy as f32 + hash::unit2(cell.0, cell.1, 2, seed);
            let dist = (fx - px).hypot(fy - py);
            if dist < best {
                (best, best_cell) = (dist, cell);
            }
        }
        match output {
            VoronoiOutput::Distance => gray(best.min(1.0)),
            VoronoiOutput::Cells => {
                let c = |ch| hash::unit2(best_cell.0, best_cell.1, ch, seed);
                [c(3), c(4), c(5), 1.0]
            }
        }
    })
}

/// Perlin / fBM gradient noise, grayscale, mapped to `[0, 1]`. The lattice wraps at
/// `scale` (a whole count) so the result tiles; fBM octaves double it, still whole.
fn noise(kind: NoiseKind, scale: f32, octaves: u32, seed: u64) -> Sampler {
    let base = count(scale);
    let octaves = match kind {
        NoiseKind::Perlin => 1,
        NoiseKind::Fbm => octaves.max(1),
    };
    Box::new(move |u, v| gray(fbm(u, v, base, octaves, seed) * 0.5 + 0.5))
}

/// Sum `octaves` Perlin octaves at doubling frequency / halving amplitude. One
/// octave is plain Perlin (octave 0's seed is the recipe seed unchanged).
fn fbm(u: f32, v: f32, base: f32, octaves: u32, seed: u64) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, base, 0.0);
    for o in 0..octaves {
        sum += amp * perlin_tiling(u, v, freq, seed.wrapping_add(o as u64 * 0x9e37));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// A single octave of tiling gradient (Perlin) noise in `[-1, 1]`. `period` is a
/// whole number; the integer lattice is taken modulo it so the field is periodic.
fn perlin_tiling(u: f32, v: f32, period: f32, seed: u64) -> f32 {
    let (x, y) = (u * period, v * period);
    let (x0, y0) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let per = period as i64;
    let corner = |cx: i64, cy: i64, lx: f32, ly: f32| {
        let (wx, wy) = (cx.rem_euclid(per), cy.rem_euclid(per));
        let ang = hash::unit2(wx, wy, 6, seed) * std::f32::consts::TAU;
        ang.cos() * lx + ang.sin() * ly
    };
    let n00 = corner(x0, y0, fx, fy);
    let n10 = corner(x0 + 1, y0, fx - 1.0, fy);
    let n01 = corner(x0, y0 + 1, fx, fy - 1.0);
    let n11 = corner(x0 + 1, y0 + 1, fx - 1.0, fy - 1.0);
    let (sx, sy) = (smoothstep(fx), smoothstep(fy));
    lerp(lerp(n00, n10, sx), lerp(n01, n11, sx), sy)
}

#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
#[path = "generators_tests.rs"]
mod tests;
