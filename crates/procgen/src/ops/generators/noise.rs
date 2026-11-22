use super::{count, finite, gray, Sampler};
use crate::hash;
use crate::ops::field::Field;
use crate::recipe::NoiseKind;

/// 2-D Perlin with unit gradients peaks at `√½`; times this, `|n|` spans `[0, 1]`.
const UNIT: f32 = std::f32::consts::SQRT_2;

#[derive(Clone, Copy, Debug)]
pub struct Octaves {
    count: u32,
    lacunarity: f32,
    gain: f32,
}

impl Octaves {
    pub fn new(count: u32, lacunarity: f32, gain: f32) -> Self {
        Self {
            count: count.max(1),
            lacunarity,
            // A negative gain would cancel octaves out.
            gain: gain.max(0.0),
        }
    }
}

/// Lattice cells either way: past a few the warp is already white noise, and the cap keeps an
/// infinite one finite.
pub const MAX_DISTORTION: f32 = 1000.0;

/// Apart from the octaves' seeds, so the push is not the noise it warps.
const WARP_SALT: [u64; 2] = [0x5bd1_e995, 0x27d4_eb2f];

pub fn sampler(
    kind: NoiseKind,
    scale: [f32; 2],
    (octave_count, lacunarity, gain): (u32, f32, Field<'_>),
    distortion: f32,
    seed: u64,
) -> Sampler<'_> {
    let base = scale.map(count);
    let push = finite(distortion, 0.0).clamp(-MAX_DISTORTION, MAX_DISTORTION);
    let warp = base.map(|b| push / b);
    Box::new(move |u, v| {
        // The gain is read before the warp: a warped read would leave the tile and clamp,
        // opening a seam.
        let gain = gain.at_pixel(u, v);
        let (u, v) = if push == 0.0 {
            (u, v)
        } else {
            let push = |w: f32, salt: u64| w * perlin_tiling(u, v, base, seed ^ salt) * UNIT;
            (
                u + push(warp[0], WARP_SALT[0]),
                v + push(warp[1], WARP_SALT[1]),
            )
        };
        let octaves = match kind {
            NoiseKind::Perlin => Octaves::new(1, 2.0, 0.5),
            _ => Octaves::new(octave_count, lacunarity, gain),
        };
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

fn sum(u: f32, v: f32, base: [f32; 2], oct: Octaves, seed: u64, shape: impl Fn(f32) -> f32) -> f32 {
    let (mut total, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..oct.count {
        let period = base.map(|b| count(b * freq));
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

pub(super) fn signed_fbm(u: f32, v: f32, period: f32, octaves: u32, seed: u64) -> f32 {
    sum(
        u,
        v,
        [period; 2],
        Octaves::new(octaves, 2.0, 0.5),
        seed,
        |n| n,
    ) * UNIT
}

fn perlin_tiling(u: f32, v: f32, period: [f32; 2], seed: u64) -> f32 {
    let (x, y) = (u * period[0], v * period[1]);
    let (x0, y0) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let per = period.map(|p| p as i64);
    let corner = |cx: i64, cy: i64, lx: f32, ly: f32| {
        let (wx, wy) = (cx.rem_euclid(per[0]), cy.rem_euclid(per[1]));
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
