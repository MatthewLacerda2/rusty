//! Gradient noise: one tiling Perlin octave and the fBM family built from it (#406).
//!
//! Every octave's lattice period is a whole count ([`count`]), so `lacunarity` may be
//! fractional without opening a seam: octave `o` runs at `count(scale × lacunarity^o)`.
//! With the defaults (`lacunarity = 2`, `gain = 0.5`) that is exactly the doubling
//! fBM rusty always baked, so older recipes bake byte-identically.

use super::{count, gray, Sampler};
use crate::procgen::hash;
use crate::procgen::recipe::NoiseKind;

/// 2-D Perlin with unit gradients peaks at `√½`; this maps `|n|` onto `[0, 1]`.
const UNIT: f32 = std::f32::consts::SQRT_2;

/// How the fBM family stacks octaves.
#[derive(Clone, Copy, Debug)]
pub struct Octaves {
    count: u32,
    lacunarity: f32,
    gain: f32,
}

impl Octaves {
    /// At least one octave; a negative `gain` would cancel octaves out, so it floors
    /// at 0 (a single octave's worth of detail).
    pub fn new(count: u32, lacunarity: f32, gain: f32) -> Self {
        Self {
            count: count.max(1),
            lacunarity,
            gain: gain.max(0.0),
        }
    }
}

/// The sampler for a [`NoiseKind`] at `scale` lattice cells per tile. `perlin` is a
/// single octave whatever `octaves` says.
pub fn sampler(kind: NoiseKind, scale: f32, octaves: Octaves, seed: u64) -> Sampler {
    let base = count(scale);
    let octaves = match kind {
        NoiseKind::Perlin => Octaves::new(1, 2.0, 0.5),
        _ => octaves,
    };
    Box::new(move |u, v| {
        let n = match kind {
            NoiseKind::Perlin | NoiseKind::Fbm => sum(u, v, base, octaves, seed, |n| n) * 0.5 + 0.5,
            NoiseKind::Ridged => sum(u, v, base, octaves, seed, |n| {
                (1.0 - (n.abs() * UNIT).min(1.0)).powi(2)
            }),
            NoiseKind::Turbulence => sum(u, v, base, octaves, seed, |n| (n.abs() * UNIT).min(1.0)),
        };
        gray(n)
    })
}

/// Sum `octaves` Perlin octaves, each passed through `shape`, normalized by the total
/// amplitude. Octave 0's seed is the recipe seed unchanged.
fn sum(u: f32, v: f32, base: f32, oct: Octaves, seed: u64, shape: impl Fn(f32) -> f32) -> f32 {
    let (mut total, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..oct.count {
        let period = count(base * freq);
        total += amp
            * shape(perlin_tiling(
                u,
                v,
                period,
                seed.wrapping_add(o as u64 * 0x9e37),
            ));
        norm += amp;
        amp *= oct.gain;
        freq *= oct.lacunarity;
    }
    total / norm
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
