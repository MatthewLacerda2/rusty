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
//! `image` is no sampler: it resamples a decoded file, whose tiling is the file's own
//! (#1491).

mod brick;
mod image;
mod noise;
mod shape;
mod voronoi;

pub use image::image;

use super::super::hash;
use super::super::image_buf::{Rgba, Size};
use super::super::recipe::{
    Brick, Checker, Constant, GradientKind, Noise, Shape, Voronoi, WaveKind,
};
use super::field::Field;

/// A generator's per-point function over the unit domain, borrowing for `'a` the
/// images its linked params read (#1494).
pub type Sampler<'a> = Box<dyn Fn(f32, f32) -> Rgba + Send + Sync + 'a>;

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
pub fn constant(p: &Constant) -> Sampler<'static> {
    let color = p.color;
    Box::new(move |_, _| color)
}

/// The `noise` family: Perlin and its fBM / ridged / turbulence sums. `gain` is the
/// op's `gain`, read at each pixel.
pub fn noise<'a>(p: &Noise, gain: Field<'a>, seed: u64) -> Sampler<'a> {
    noise::sampler(p.kind, p.scale, (p.octaves, p.lacunarity, gain), seed)
}

/// The Voronoi / Worley cellular pattern. `randomness` is the op's, read per cell.
pub fn voronoi<'a>(p: &Voronoi, randomness: Field<'a>, seed: u64) -> Sampler<'a> {
    voronoi::sampler(p.scale, p.output, p.metric, randomness, seed)
}

/// The running-bond brick pattern.
pub fn brick(p: &Brick, seed: u64) -> Sampler<'static> {
    brick::sampler(p.rows, p.cols, p.mortar, p.output, seed)
}

/// One centred shape per tile. `softness` is the op's, read at each pixel.
pub fn shape<'a>(p: &Shape, softness: Field<'a>) -> Sampler<'a> {
    shape::sampler(p.kind, p.size, p.roundness, softness)
}

/// Per-pixel uniform white noise, grayscale. The pixel lattice wraps at `size`, per
/// axis. `size` matters only here: the grain is one pixel on each axis.
pub fn white_noise(size: Size, seed: u64) -> Sampler<'static> {
    let (rx, ry) = (size.width.max(1) as i64, size.height.max(1) as i64);
    let (w, h) = (size.width as f32, size.height as f32);
    Box::new(move |u, v| {
        let x = ((u * w).floor() as i64).rem_euclid(rx);
        let y = ((v * h).floor() as i64).rem_euclid(ry);
        gray(hash::unit2(x, y, 0, seed))
    })
}

/// Checkerboard: `tiles` squares across each axis; alternating `a`/`b`. `tiles`
/// rounds **up to even**: an odd count would put two same-colour squares side by
/// side at the wrap.
pub fn checker(p: &Checker) -> Sampler<'static> {
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
pub fn gradient(kind: GradientKind) -> Sampler<'static> {
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
pub fn wave(kind: WaveKind, frequency: f32) -> Sampler<'static> {
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
