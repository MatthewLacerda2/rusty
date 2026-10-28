//! src/procgen/ops/generators/mod.rs — source ops (no inputs).
//!
//! Every generator is a pure **sampler** `(u, v) -> Rgba` over the unit domain
//! `[0, 1)²`, built from its op's params (one function per op: [`noise()`],
//! [`checker`], …), which [`super::eval_node`] paints per pixel. They are seamless
//! by construction because every periodic param is a **count of periods per tile**
//! ([`count`]): noise/Voronoi wrap an integer lattice through [`super::super::hash`]
//! (the `noise` and `voronoi` families, #406), and wave/brick/checker repeat a whole
//! number of times (#392); `shape` is symmetric about the tile centre, so it meets
//! itself at the wrap (#407). Two generators are the documented exceptions:
//! `gradient linear` is a one-way ramp (use `linear_tiling` for a seamless one), and
//! the radial shapes (`gradient radial`, `wave rings`) meet the edge
//! mirror-symmetrically — continuous across the seam, not periodic beyond it.

mod brick;
mod noise;
mod shape;
mod voronoi;

use super::super::hash;
use super::super::image_buf::Rgba;
use super::super::recipe::{
    Brick, Checker, Constant, GradientKind, Noise, Shape, Voronoi, WaveKind,
};

/// A generator's per-point function over the unit domain.
pub type Sampler = Box<dyn Fn(f32, f32) -> Rgba + Send + Sync>;

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

/// A flat color fill.
pub fn constant(p: &Constant) -> Sampler {
    let color = p.color;
    Box::new(move |_, _| color)
}

/// The `noise` family: Perlin and its fBM / ridged / turbulence sums.
pub fn noise(p: &Noise, seed: u64) -> Sampler {
    let octaves = noise::Octaves::new(p.octaves, p.lacunarity, p.gain);
    noise::sampler(p.kind, p.scale, octaves, seed)
}

/// The Voronoi / Worley cellular pattern.
pub fn voronoi(p: &Voronoi, seed: u64) -> Sampler {
    voronoi::sampler(p.scale, p.output, p.randomness, seed)
}

/// The running-bond brick pattern.
pub fn brick(p: &Brick, seed: u64) -> Sampler {
    brick::sampler(p.rows, p.cols, p.mortar, p.output, seed)
}

/// One centred shape per tile.
pub fn shape(p: &Shape) -> Sampler {
    shape::sampler(p.kind, p.size, p.roundness, p.softness)
}

/// Per-pixel uniform white noise, grayscale. The pixel lattice wraps at `resolution`.
/// `resolution` matters only here: the grain is one pixel.
pub fn white_noise(resolution: u32, seed: u64) -> Sampler {
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
pub fn checker(p: &Checker) -> Sampler {
    let (a, b) = (p.color_a, p.color_b);
    let t = p.tiles.max(1).next_multiple_of(2) as f32;
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
pub fn gradient(kind: GradientKind) -> Sampler {
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
pub fn wave(kind: WaveKind, frequency: f32) -> Sampler {
    let f = count(frequency);
    Box::new(move |u, v| {
        let phase = match kind {
            WaveKind::Bands => u * f,
            WaveKind::Rings => (u - 0.5).hypot(v - 0.5) * f,
        };
        gray(0.5 + 0.5 * (phase * std::f32::consts::TAU).sin())
    })
}

#[cfg(test)]
mod tests;
