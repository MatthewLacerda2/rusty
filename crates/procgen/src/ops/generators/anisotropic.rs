//! Anisotropic value noise (#1740): fibres along a lattice direction, so they tile.
//!
//! A direction `(a, b)` of whole numbers gives two coordinates, `s = a·u + b·v`
//! along the fibres and `t = −b·u + a·v` across them. The noise is a lattice of
//! cells in `(s, t)`, `along` cells per unit of `s` and `across` per unit of `t`,
//! and a step of one tile moves a point by a whole number of cells on each axis:
//! `P = (a·along, −b·across)` for a step in `u`, `Q = (b·along, a·across)` in `v`.
//! A cell's value is hashed after reducing it modulo `P` and `Q` (`Lattice::reduce`), so the
//! field repeats exactly once per tile, at any of those directions.
//!
//! The asked angle snaps to the nearest direction with both components at most
//! [`MAX_STEP`] whose length `L = √(a² + b²)` is at most `scale / anisotropy`
//! (the axes and diagonals always fit): a fibre crosses `1 / L` of a tile in `s`
//! per cell, so a longer step would cut the fibres shorter than asked. The snap is
//! within 3.6° of the angle once `scale / anisotropy ≥ 8.1`, 5.7° at 6, 9.2° at 3.2,
//! and 22.5° (axes and diagonals only) under 2.3.
//!
//! On an axis, `P` and `Q` are `(along, 0)` and `(0, across)`, so one cell along
//! would hash every cell of a fibre alike: ruler-straight stripes. `along` floors
//! at 2 there, so a fibre on an axis is at most half a tile long.

use std::f32::consts::{PI, SQRT_2, TAU};

use super::{count, finite, gray, Sampler};
use crate::hash;
use crate::recipe::AnisotropicNoise;

/// The largest component of a snap direction.
pub const MAX_STEP: i64 = 8;

/// The most cells a lattice axis takes: far past one per pixel at 2048², and finite.
pub const MAX_CELLS: f32 = 4096.0;

/// The fibre sampler: `scale` fibres across the tile, `anisotropy` (at least 1)
/// times as long, at `angle` turns snapped by [`snap`]. NaN params read their
/// defaults (anisotropy 8, angle 0); counts are capped at [`MAX_CELLS`], and
/// on an axis there are at least 2 cells along the fibre.
pub fn sampler(p: &AnisotropicNoise, seed: u64) -> Sampler<'static> {
    let scale = count(p.scale).min(MAX_CELLS);
    let aniso = finite(p.anisotropy, 8.0).clamp(1.0, MAX_CELLS);
    let (a, b) = snap(finite(p.angle, 0.0), scale / aniso);
    let len = ((a * a + b * b) as f32).sqrt();
    let across = count(scale / len).min(MAX_CELLS);
    // On an axis a single cell along would repeat every cell: straight stripes.
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

/// The tiling direction nearest `angle` turns among those no longer than `limit`
/// (or a diagonal), as whole `(a, b)` with `b ≥ 0` (a fibre has no front: `angle`
/// and `angle + ½` are the same).
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

/// The cell lattice and the two whole-cell periods a tile step moves it by.
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

    /// Cell `(i, j)` moved by whole periods into the one cell of its class the hash
    /// reads: `(i, j) − ⌊x⌋·P − ⌊y⌋·Q`, where `(x, y)` are its coordinates in `P`
    /// and `Q`. Two cells a whole tile apart reduce to the same cell.
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

/// Value noise at `(s, t)` on `lattice`, smoothstepped across each cell; in
/// `[0, 1)`.
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
