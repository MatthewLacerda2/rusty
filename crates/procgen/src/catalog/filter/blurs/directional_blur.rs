use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::recipe::{op, opt, req, Lit, OpInfo, ONE};

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

/// The longest streak, in tiles: a bound on cost, not a period, since off an axis a line
/// closes later (√2 tiles at 45°).
const MAX_LENGTH: f32 = 1.0;
/// The most samples one pass reads. Past it the box is a `len / k` box, then `k` copies of
/// it end to end (`k ≈ √n`).
const ONE_PASS: usize = 64;

pub fn directional_blur(img: &Image, length: f32, angle: f32) -> Image {
    if length.is_nan() || length <= 0.0 {
        return img.clone();
    }
    let len = length.min(MAX_LENGTH);
    // Folded to one turn first, so a huge angle can't overflow `× TAU` to NaN.
    let turn = if angle.is_finite() {
        angle.rem_euclid(1.0)
    } else {
        0.0
    };
    let (sin, cos) = (turn * std::f32::consts::TAU).sin_cos();
    let n = ((len * img.size().longer() as f32).ceil() as usize).max(2);
    if n <= ONE_PASS {
        return smear(img, &line(n, len, (cos, sin)));
    }
    let k = (n as f32).sqrt().ceil() as usize;
    let m = n.div_ceil(k);
    let short = smear(img, &line(m, len / k as f32, (cos, sin)));
    smear(&short, &line(k, len, (cos, sin)))
}

fn line(n: usize, len: f32, (du, dv): (f32, f32)) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let t = ((i as f32 + 0.5) / n as f32 - 0.5) * len;
            (du * t, dv * t)
        })
        .collect()
}

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
