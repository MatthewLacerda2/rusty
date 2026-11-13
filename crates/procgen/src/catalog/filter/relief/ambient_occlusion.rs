//! The `ambient_occlusion` op: occlusion from a height map by horizon search
//! (Substance Designer's *Ambient Occlusion (HBAO)*, #1743).
//!
//! `cavity` darkens only what sits below a blur of itself: a narrow crevice. A
//! horizon search also darkens the foot of a tall shape and the floor of a wide
//! groove, because it asks how much sky each pixel sees. From every pixel it walks
//! `quality` directions, [`STEPS`] samples each out to `radius`, and keeps the
//! steepest rise it meets; the sky a direction loses is that horizon angle's sine.
//! The directions and samples are fixed (no jitter), so the cost is
//! `pixels × quality × STEPS` bilinear reads and a recipe bakes byte-identically.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::recipe::{op, opt, req, Lit, OpInfo, ONE};

/// Height (red) → occlusion: white where a pixel sees the whole sky, darker the
/// more of it the relief within `radius` (tiles) hides. `depth` is how tall white
/// stands, in tiles; `quality` is how many directions are searched (1 to
/// 64). Reads the input's red; writes grey with alpha 1. `radius` caps at half a
/// tile. Wraps, so it tiles; samples land in tile units, so it looks the same at
/// any resolution.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmbientOcclusion {
    pub radius: f32,
    #[serde(default = "depth")]
    pub depth: f32,
    #[serde(default = "quality")]
    pub quality: u32,
}

impl Row for AmbientOcclusion {
    const INFO: OpInfo = op(
        "ambient_occlusion",
        "filter",
        ONE,
        &[
            req("radius", "number"),
            opt("depth", "number", Lit::Number(DEPTH)),
            opt("quality", "integer", Lit::Integer(QUALITY)),
        ],
    );
}

/// Samples along each direction, evenly out to the radius.
const STEPS: u32 = 16;
/// The most directions a node searches: bounds its cost.
const MAX_QUALITY: u32 = 64;
const DEPTH: f32 = 0.1;
const QUALITY: u32 = 8;

impl Op for AmbientOcclusion {
    fn run(&self, cx: &Cx) -> Image {
        occlusion(cx.input(0), self.radius, self.depth, self.quality)
    }
}

/// The occlusion of `img`'s red as a height `depth` tiles tall at 1, searched
/// `radius` tiles out in `quality` directions. A radius or depth of 0 or less
/// (or NaN) sees the whole sky: white. The radius caps at half a tile, as
/// `blur`'s does: past it the search only wraps onto what it has read.
pub fn occlusion(img: &Image, radius: f32, depth: f32, quality: u32) -> Image {
    if !(radius > 0.0 && depth > 0.0) {
        return Image::filled(img.size(), [1.0; 4]);
    }
    let radius = radius.min(0.5);
    let dirs = quality.clamp(1, MAX_QUALITY);
    let rays: Vec<(f32, f32)> = (0..dirs)
        .map(|k| {
            let a = std::f32::consts::TAU * (k as f32 + 0.5) / dirs as f32;
            (a.cos(), a.sin())
        })
        .collect();
    let heights = Heights::of(img);
    let h = |u: f32, v: f32| heights.sample(u, v);
    Image::fill_uv(img.size(), |u, v| {
        let here = h(u, v);
        let lost: f32 = rays
            .iter()
            .map(|&(du, dv)| {
                let rise = (1..=STEPS)
                    .map(|s| {
                        let d = radius * s as f32 / STEPS as f32;
                        (h(u + du * d, v + dv * d) - here) * depth / d
                    })
                    .fold(0.0f32, f32::max);
                rise / (1.0 + rise * rise).sqrt()
            })
            .sum();
        let ao = 1.0 - lost / dirs as f32;
        [ao, ao, ao, 1.0]
    })
}

/// The input's red alone, sampled as [`Image::sample_bilinear_wrapped`] samples:
/// the search reads it `quality × STEPS` times a pixel, and a quarter of the bytes
/// keeps more of the reach in cache (about a quarter faster at 2048²).
struct Heights {
    w: i64,
    h: i64,
    red: Vec<f32>,
}

impl Heights {
    fn of(img: &Image) -> Self {
        let (w, h) = (i64::from(img.width()), i64::from(img.height()));
        let red = img.pixels().iter().map(|p| p[0]).collect();
        Heights { w, h, red }
    }

    fn at(&self, x: i64, y: i64) -> f32 {
        self.red[(y.rem_euclid(self.h) * self.w + x.rem_euclid(self.w)) as usize]
    }

    /// Bilinear between the four nearest pixel centres, wrapped.
    fn sample(&self, u: f32, v: f32) -> f32 {
        let (x, y) = (u * self.w as f32 - 0.5, v * self.h as f32 - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let top = self.at(xi, yi) + (self.at(xi + 1, yi) - self.at(xi, yi)) * tx;
        let low = self.at(xi, yi + 1) + (self.at(xi + 1, yi + 1) - self.at(xi, yi + 1)) * tx;
        top + (low - top) * ty
    }
}

fn depth() -> f32 {
    DEPTH
}

fn quality() -> u32 {
    QUALITY
}

#[cfg(test)]
#[path = "ambient_occlusion_tests.rs"]
mod tests;
