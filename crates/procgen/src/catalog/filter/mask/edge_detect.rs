use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::distance;
use crate::ops::field::Field;
use crate::recipe::{op, opt, req, Lit, OpInfo, ONE};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeDetect {
    pub width: f32,
    #[serde(default)]
    pub roundness: f32,
}

impl Row for EdgeDetect {
    const INFO: OpInfo = op(
        "edge_detect",
        "filter",
        ONE,
        &[
            req("width", "number"),
            opt("roundness", "number", Lit::Number(0.0)),
        ],
    );
}

impl Op for EdgeDetect {
    fn run(&self, cx: &Cx) -> Image {
        let img = cx.input(0);
        let unit = img.size().longer() as f32;
        // The half-width in pixels, capped at a tile (NaN and ≤ 0 read no line).
        let half = (self.width / 2.0 * unit).max(0.0).min(unit);
        // `distance` clamps at its radius, so it reads a pixel past the half-width: every
        // pixel the line touches gets its true distance.
        let reach = (half + 1.0) / unit;
        let inside = distance::distance(img, Field::Const(reach));
        let outside = distance::distance(&complement(img), Field::Const(reach));
        let round = self.roundness.clamp(0.0, 1.0);
        let mut out = Image::new(img.size());
        out.for_each_row(|y, row| {
            let (src, a, b) = (img.row(y), inside.row(y), outside.row(y));
            for (x, o) in row.iter_mut().enumerate() {
                let t = if src[x][0] > 0.5 { a[x][0] } else { b[x][0] };
                let v = covered(t * (half + 1.0), half, round);
                *o = [v, v, v, 1.0];
            }
        });
        out
    }
}

fn complement(img: &Image) -> Image {
    let mut out = img.clone();
    out.map_in_place(|p| [f32::from(u8::from(p[0] <= 0.5)), 0.0, 0.0, 1.0]);
    out
}

fn covered(e: f32, half: f32, roundness: f32) -> f32 {
    let (near, far) = (e - 0.5, (e + 0.5).min(half));
    let share = (far - near).clamp(0.0, 1.0);
    if share == 0.0 {
        return 0.0;
    }
    share * profile((near + far) / 2.0 / half, roundness)
}

fn profile(t: f32, roundness: f32) -> f32 {
    let round = (1.0 - t * t).max(0.0).sqrt();
    1.0 + (round - 1.0) * roundness
}

#[cfg(test)]
#[path = "edge_detect_tests.rs"]
mod tests;
