//! The `slope_blur` op: smear an image along another's slopes (Substance
//! Designer's *Slope Blur*, #1739): erosion, chipped paint edges, grime running
//! off a shape.
//!
//! At each pixel the slope map's gradient (its red, by central differences, in
//! height per tile) points uphill. The op reads input 1 at `samples` points
//! evenly along `intensity × gradient` from the pixel and keeps their mean, min or
//! max, so each pixel takes what lies uphill of it: the image runs down the
//! slopes, and flat ground (no gradient) is left as it is. A fixed sample count,
//! bilinear reads and a reach in tile units keep a low-`resolution` preview
//! reading like the final.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::{Image, Rgba};
use crate::procgen::recipe::{arity, choice, op, opt, req, Lit, OpInfo, OpParam};

/// How the samples along the slope combine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlopeMode {
    /// Their mean: a smear.
    #[default]
    Blur,
    /// Their darkest, per channel: dark grows down the slopes.
    Min,
    /// Their brightest, per channel: bright runs down the slopes.
    Max,
}

/// Input 1 smeared down the slopes of input 2's red: `samples` reads (1 to 64)
/// along `intensity × gradient` tiles, the gradient in height per tile, combined
/// by `mode`. A negative intensity smears up the slopes. The reach caps at one
/// tile; a NaN intensity passes input 1 through. Wraps, so it tiles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlopeBlur {
    pub intensity: f32,
    #[serde(default = "samples")]
    pub samples: u32,
    #[serde(default)]
    pub mode: SlopeMode,
}

impl Row for SlopeBlur {
    const INFO: OpInfo = op(
        "slope_blur",
        "filter",
        arity(2, 2),
        &[
            req("intensity", "number"),
            opt("samples", "integer", Lit::Integer(SAMPLES)),
            OpParam {
                default: Some(Lit::Text("blur")),
                ..choice("mode", &["blur", "min", "max"])
            },
        ],
    );
}

impl Op for SlopeBlur {
    fn run(&self, cx: &Cx) -> Image {
        let p = self;
        slope_blur(cx.input(0), cx.input(1), p.intensity, p.samples, p.mode)
    }
}

const SAMPLES: u32 = 8;
/// The most samples a node reads per pixel: bounds its cost.
pub const MAX_SAMPLES: u32 = 64;
/// The longest reach, in tiles.
const MAX_REACH: f32 = 1.0;
/// The largest intensity read: past it any slope already reaches [`MAX_REACH`],
/// and a bound keeps `intensity × 0` a 0 (an infinite one would read NaN).
pub const MAX_INTENSITY: f32 = 1e6;

fn samples() -> u32 {
    SAMPLES
}

/// `img` smeared along `intensity ×` the gradient of `slope`'s red.
pub fn slope_blur(img: &Image, slope: &Image, intensity: f32, n: u32, mode: SlopeMode) -> Image {
    if intensity.is_nan() || intensity == 0.0 {
        return img.clone();
    }
    let intensity = intensity.clamp(-MAX_INTENSITY, MAX_INTENSITY);
    let n = n.clamp(1, MAX_SAMPLES);
    let (w, h) = (img.width() as f32, img.height() as f32);
    Image::fill_uv(img.size(), |u, v| {
        let (x, y) = ((u * w) as i64, (v * h) as i64);
        let s = |dx, dy| slope.get_wrapped(x + dx, y + dy)[0];
        let grad = (
            (s(1, 0) - s(-1, 0)) * w / 2.0,
            (s(0, 1) - s(0, -1)) * h / 2.0,
        );
        let (du, dv) = reach(intensity, grad);
        if du == 0.0 && dv == 0.0 {
            // Flat ground: every sample is this pixel, read exactly.
            return img.get_wrapped(x, y);
        }
        let mut acc = first(mode);
        for i in 0..n {
            let t = (i as f32 + 0.5) / n as f32;
            let p = img.sample_bilinear_wrapped(u + du * t, v + dv * t);
            for c in 0..4 {
                acc[c] = match mode {
                    SlopeMode::Blur => acc[c] + p[c],
                    SlopeMode::Min => acc[c].min(p[c]),
                    SlopeMode::Max => acc[c].max(p[c]),
                };
            }
        }
        match mode {
            SlopeMode::Blur => acc.map(|a| a / n as f32),
            _ => acc,
        }
    })
}

/// `intensity × grad`, shortened to [`MAX_REACH`]; a non-finite one (an infinite
/// height in the slope map) reads 0.
fn reach(intensity: f32, (gu, gv): (f32, f32)) -> (f32, f32) {
    let (du, dv) = (intensity * gu, intensity * gv);
    let len = du.hypot(dv);
    if !len.is_finite() {
        (0.0, 0.0)
    } else if len > MAX_REACH {
        (du * MAX_REACH / len, dv * MAX_REACH / len)
    } else {
        (du, dv)
    }
}

/// What `mode` folds its samples from.
fn first(mode: SlopeMode) -> Rgba {
    match mode {
        SlopeMode::Blur => [0.0; 4],
        SlopeMode::Min => [f32::INFINITY; 4],
        SlopeMode::Max => [f32::NEG_INFINITY; 4],
    }
}

#[cfg(test)]
#[path = "slope_blur_tests.rs"]
mod tests;
