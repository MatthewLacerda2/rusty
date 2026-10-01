//! src/procgen/ops/generators/mod.rs — source ops (no inputs).
//!
//! Every generator is a pure **sampler** `(u, v) -> Rgba` over the unit domain
//! `[0, 1)²`; [`sampler`] builds one from an [`OpKind`] and the runner paints it per
//! pixel. They are seamless by construction because every periodic param is a
//! **count of periods per tile** ([`count`]): noise/Voronoi wrap an integer lattice
//! through [`super::super::hash`] (the `noise` and `voronoi` families, #406), and
//! wave/brick/checker repeat a whole number of times (#392). Two generators are the
//! documented exceptions: `gradient linear` is a one-way ramp (use `linear_tiling`
//! for a seamless one), and the radial shapes (`gradient radial`, `wave rings`) meet
//! the edge mirror-symmetrically — continuous across the seam, not periodic beyond it.

mod noise;
mod voronoi;

use super::super::hash;
use super::super::image_buf::Rgba;
use super::super::recipe::{GradientKind, OpKind, WaveKind};

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
            lacunarity,
            gain,
        } => noise::sampler(
            kind,
            scale,
            noise::Octaves::new(octaves, lacunarity, gain),
            seed,
        ),
        OpKind::Voronoi {
            scale,
            output,
            randomness,
        } => voronoi::sampler(scale, output, randomness, seed),
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

#[cfg(test)]
mod tests;
