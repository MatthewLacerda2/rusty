//! The `directional_blur` op: a motion blur along an angle (Substance Designer's
//! *Directional Blur*, #1739).
//!
//! `mapping` can only stretch a noise along the axes in a tiling bake, since a
//! rotation off a quarter turn does not map the tile onto itself. A blur along a
//! line is a convolution, and a wrapped convolution tiles at any angle: each
//! pixel averages bilinear samples spaced evenly along the line, and the wrap
//! folds the line back into the tile.
//!
//! The samples are about a pixel apart, so the streak is smooth. Past
//! [`ONE_PASS`] samples the box is two passes: a short box of `len / k`, then `k`
//! copies of it laid end to end (`k ≈ √n`), whose sample points are the same
//! evenly spaced `n`, read for `2√n` reads a pixel instead of `n`.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::recipe::{op, opt, req, Lit, OpInfo, ONE};

/// Average of the input along a line `length` tiles long, centred on each pixel and
/// turned `angle` turns from the horizontal (0 smears left–right, 0.25 up–down,
/// 0.125 along the diagonal). Brushed metal, rain streaks, wiped surfaces. Wraps,
/// so it tiles at any angle. Every channel, alpha included. A `length` of 0 or less
/// (or NaN) passes the input through; it caps at one tile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectionalBlur {
    pub length: f32,
    #[serde(default)]
    pub angle: f32,
}

impl Row for DirectionalBlur {
    const INFO: OpInfo = op(
        "directional_blur",
        "filter",
        ONE,
        &[
            req("length", "number"),
            opt("angle", "number", Lit::Number(0.0)),
        ],
    );
}

impl Op for DirectionalBlur {
    fn run(&self, cx: &Cx) -> Image {
        directional_blur(cx.input(0), self.length, self.angle)
    }
}

/// The longest streak, in tiles: past one tile a line only re-reads the tile.
const MAX_LENGTH: f32 = 1.0;
/// The most samples one pass reads; a longer streak runs as two.
const ONE_PASS: usize = 64;

/// `img` averaged along a centred line `length` tiles long at `angle` turns.
pub fn directional_blur(img: &Image, length: f32, angle: f32) -> Image {
    if !(length > 0.0) {
        return img.clone();
    }
    let len = length.min(MAX_LENGTH);
    let turn = if angle.is_finite() { angle } else { 0.0 };
    let (sin, cos) = (turn * std::f32::consts::TAU).sin_cos();
    // Samples about a pixel apart, two at least so a sub-pixel streak still acts.
    let n = ((len * img.size().longer() as f32).ceil() as usize).max(2);
    if n <= ONE_PASS {
        return smear(img, &line(n, len, (cos, sin)));
    }
    let k = (n as f32).sqrt().ceil() as usize;
    let m = n.div_ceil(k);
    let short = smear(img, &line(m, len / k as f32, (cos, sin)));
    smear(&short, &line(k, len, (cos, sin)))
}

/// `n` offsets at the midpoints of `n` equal parts of a centred line `len` long
/// along `dir`, in tile units.
fn line(n: usize, len: f32, (du, dv): (f32, f32)) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let t = ((i as f32 + 0.5) / n as f32 - 0.5) * len;
            (du * t, dv * t)
        })
        .collect()
}

/// The mean of `img` sampled at each pixel plus each offset, bilinear and wrapped.
fn smear(img: &Image, offsets: &[(f32, f32)]) -> Image {
    let w = 1.0 / offsets.len() as f32;
    Image::fill_uv(img.size(), |u, v| {
        let mut sum = [0.0f32; 4];
        for &(du, dv) in offsets {
            let p = img.sample_bilinear_wrapped(u + du, v + dv);
            for c in 0..4 {
                sum[c] += p[c];
            }
        }
        sum.map(|s| s * w)
    })
}

#[cfg(test)]
#[path = "directional_blur_tests.rs"]
mod tests;
