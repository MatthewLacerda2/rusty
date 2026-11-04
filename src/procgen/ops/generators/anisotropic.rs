//! Anisotropic value noise (#1740): fibres along a lattice direction, so they tile.
//!
//! A direction `(a, b)` of whole numbers gives two coordinates, `s = a·u + b·v`
//! along the fibres and `t = −b·u + a·v` across them, and a step of one tile on
//! either axis moves both by whole numbers. Noise whose lattice wraps at a whole
//! count of cells per unit of `s` and of `t` therefore repeats every tile, at any
//! of those directions. The asked angle snaps to the nearest one: the directions
//! with both components at most [`MAX_STEP`] whose length fits under `scale` (a
//! longer one would pack more fibres across the tile than asked), so at scale 11 and
//! up the snap is within 3.6° (half the 7.1° gap beside each axis, the widest), and
//! at a low scale only the axes and the diagonals are left.

use std::f32::consts::{PI, TAU};

use super::{count, finite, gray, Sampler};
use crate::procgen::hash;
use crate::procgen::recipe::AnisotropicNoise;

/// The largest component of a snap direction.
pub const MAX_STEP: i64 = 8;

/// The most cells a lattice axis takes: far past one per pixel at 2048², and finite.
pub const MAX_CELLS: f32 = 4096.0;

/// The fibre sampler: `scale` fibres across the tile, `anisotropy` (at least 1)
/// times as long, at `angle` turns snapped by [`snap`]. NaN params read their
/// defaults (anisotropy 8, angle 0); counts are capped at [`MAX_CELLS`].
pub fn sampler(p: &AnisotropicNoise, seed: u64) -> Sampler<'static> {
    let scale = count(p.scale).min(MAX_CELLS);
    let aniso = finite(p.anisotropy, 8.0).clamp(1.0, MAX_CELLS);
    let (a, b) = snap(finite(p.angle, 0.0), scale);
    let len = ((a * a + b * b) as f32).sqrt();
    let across = count(scale / len).min(MAX_CELLS);
    let along = count(scale / (aniso * len)).min(MAX_CELLS);
    let (af, bf) = (a as f32, b as f32);
    Box::new(move |u, v| {
        let s = (af * u + bf * v) * along;
        let t = (af * v - bf * u) * across;
        gray(value(s, t, (along as i64, across as i64), seed))
    })
}

/// The tiling direction nearest `angle` turns among those that fit `scale`, as
/// whole `(a, b)` with `b ≥ 0` (a fibre has no front: `angle` and `angle + ½` are
/// the same).
pub fn snap(angle: f32, scale: f32) -> (i64, i64) {
    let want = (angle * TAU).rem_euclid(PI);
    let mut best = ((1, 0), f32::MAX);
    for a in -MAX_STEP..=MAX_STEP {
        for b in 0..=MAX_STEP {
            let len = ((a * a + b * b) as f32).sqrt();
            if gcd(a.abs(), b) != 1 || (b == 0 && a < 0) || len > scale.max(1.0) {
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

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Value noise at `(s, t)` on a lattice of `per` cells that wraps, smoothstepped
/// across each cell; in `[0, 1)`.
fn value(s: f32, t: f32, per: (i64, i64), seed: u64) -> f32 {
    let (i, j) = (s.floor(), t.floor());
    let (fs, ft) = (smooth(s - i), smooth(t - j));
    let (i, j) = (i as i64, j as i64);
    let at = |di: i64, dj: i64| {
        let (x, y) = ((i + di).rem_euclid(per.0), (j + dj).rem_euclid(per.1));
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
