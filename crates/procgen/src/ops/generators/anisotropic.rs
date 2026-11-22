//! A whole direction `(a, b)` makes a one-tile step move the `(s, t)` cell lattice by whole
//! cells, `P = (a·along, −b·across)` along `u` and `Q = (b·along, a·across)` along `v`. A cell
//! is hashed after reducing it modulo `P` and `Q`, so the field repeats once per tile.

use std::f32::consts::{PI, SQRT_2, TAU};

use super::{count, finite, gray, Sampler};
use crate::hash;
use crate::recipe::AnisotropicNoise;

pub const MAX_STEP: i64 = 8;

/// Cells per lattice axis: far past one per pixel at 2048², and finite.
pub const MAX_CELLS: f32 = 4096.0;

pub fn sampler(p: &AnisotropicNoise, seed: u64) -> Sampler<'static> {
    let scale = count(p.scale).min(MAX_CELLS);
    let aniso = finite(p.anisotropy, 8.0).clamp(1.0, MAX_CELLS);
    let (a, b) = snap(finite(p.angle, 0.0), scale / aniso);
    let len = ((a * a + b * b) as f32).sqrt();
    let across = count(scale / len).min(MAX_CELLS);
    // On an axis one cell along would hash a whole fibre alike: ruler-straight stripes.
    let floor = if a == 0 || b == 0 { 2.0 } else { 1.0 };
    let along = count(scale / (aniso * len)).clamp(floor, MAX_CELLS);
    let lattice = Lattice::new((a, b), (along as i64, across as i64));
    let (af, bf) = (a as f32, b as f32);
    Box::new(move |u, v| {
        let s = (af * u + bf * v) * along;
        let t = (af * v - bf * u) * across;
        gray(value(s, t, &lattice, seed))
    })
}

/// `b ≥ 0`: a fibre has no front, so `angle` and `angle + ½` are one direction. A diagonal
/// always fits under `limit`.
pub fn snap(angle: f32, limit: f32) -> (i64, i64) {
    let want = (angle * TAU).rem_euclid(PI);
    let limit = limit.max(SQRT_2 + 1e-4);
    let mut best = ((1, 0), f32::MAX);
    for a in -MAX_STEP..=MAX_STEP {
        for b in 0..=MAX_STEP {
            let len = ((a * a + b * b) as f32).sqrt();
            if gcd(a.abs(), b) != 1 || (b == 0 && a < 0) || len > limit {
                continue;
            }
            let at = (b as f32).atan2(a as f32);
            let off = (at - want).abs();
            let off = off.min(PI - off);
            if off < best.1 {
                best = ((a, b), off);
            }
        }
    }
    best.0
}

struct Lattice {
    p: (i64, i64),
    q: (i64, i64),
    det: i64,
}

impl Lattice {
    fn new((a, b): (i64, i64), (along, across): (i64, i64)) -> Self {
        Self {
            p: (a * along, -b * across),
            q: (b * along, a * across),
            det: (a * a + b * b) * along * across,
        }
    }

    fn reduce(&self, (i, j): (i64, i64)) -> (i64, i64) {
        let (p, q) = (self.p, self.q);
        let x = (q.1 * i - q.0 * j).div_euclid(self.det);
        let y = (p.0 * j - p.1 * i).div_euclid(self.det);
        (i - x * p.0 - y * q.0, j - x * p.1 - y * q.1)
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn value(s: f32, t: f32, lattice: &Lattice, seed: u64) -> f32 {
    let (i, j) = (s.floor(), t.floor());
    let (fs, ft) = (smooth(s - i), smooth(t - j));
    let (i, j) = (i as i64, j as i64);
    let at = |di: i64, dj: i64| {
        let (x, y) = lattice.reduce((i + di, j + dj));
        hash::unit2(x, y, 13, seed)
    };
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * fs;
    let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * fs;
    top + (bottom - top) * ft
}

#[inline]
fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
#[path = "anisotropic_tests.rs"]
mod tests;
