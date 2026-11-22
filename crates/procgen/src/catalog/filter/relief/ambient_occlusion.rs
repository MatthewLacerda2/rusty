use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::recipe::{op, opt, req, Lit, OpInfo, ONE};

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

const STEPS: u32 = 16;
const MAX_QUALITY: u32 = 64;
const DEPTH: f32 = 0.1;
const QUALITY: u32 = 8;

impl Op for AmbientOcclusion {
    fn run(&self, cx: &Cx) -> Image {
        occlusion(cx.input(0), self.radius, self.depth, self.quality)
    }
}

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
