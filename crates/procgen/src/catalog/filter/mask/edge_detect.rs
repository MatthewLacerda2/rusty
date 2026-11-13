//! The `edge_detect` op: the outline of a mask's shapes (Substance Designer's
//! *Edge Detect*, #1743).

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::distance;
use crate::ops::field::Field;
use crate::recipe::{op, opt, req, Lit, OpInfo, ONE};

/// Mask → its outline: red > 0.5 is inside, as `distance` reads it, and a line
/// `width` tiles wide straddles every edge, half inside and half out. `roundness`
/// shapes the line's profile, from flat-topped (0) to a round bead (1, a profile
/// that bevels as a height). A pixel the line only partly covers reads the share
/// it covers, so a line thinner than a pixel at a low `resolution` fades rather
/// than vanishing. Reads the input's red; writes grey with alpha 1. Wraps, so a
/// tiling mask gives tiling lines; a mask with no edge reads black.
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
        // `distance` clamps at its radius, so it reads a pixel past the half-width:
        // every pixel the line touches gets its true distance.
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

/// The mask's complement as `distance` reads it: red > 0.5 exactly where `img`'s
/// is not.
fn complement(img: &Image) -> Image {
    let mut out = img.clone();
    out.map_in_place(|p| [f32::from(u8::from(p[0] <= 0.5)), 0.0, 0.0, 1.0]);
    out
}

/// A pixel's value: the share of it the line covers, times the line's height
/// there. `e` is the pixel centre's distance from the edge and `half` the line's
/// half-width, both in pixels. The pixel spans `[e − ½, e + ½]` across the edge and
/// the line `[0, half]`, so a line thinner than a pixel fades rather than vanishing,
/// and the line's area, and so the map's mean, holds at any resolution (#1743).
fn covered(e: f32, half: f32, roundness: f32) -> f32 {
    let (near, far) = (e - 0.5, (e + 0.5).min(half));
    let share = (far - near).clamp(0.0, 1.0);
    if share == 0.0 {
        return 0.0;
    }
    share * profile((near + far) / 2.0 / half, roundness)
}

/// The line's height at `t`, the distance from the edge over half the width
/// (`[0, 1]`): 1 across the line when flat, `√(1 − t²)` when round.
fn profile(t: f32, roundness: f32) -> f32 {
    let round = (1.0 - t * t).max(0.0).sqrt();
    1.0 + (round - 1.0) * roundness
}

#[cfg(test)]
#[path = "edge_detect_tests.rs"]
mod tests;
