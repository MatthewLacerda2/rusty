//! The `edge_detect` op: the outline of a mask's shapes (Substance Designer's
//! *Edge Detect*, #1743).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::distance;
use crate::procgen::ops::field::Field;
use crate::procgen::recipe::{op, opt, req, Lit, OpInfo, ONE};

/// Mask → its outline: red > 0.5 is inside, as `distance` reads it, and a line
/// `width` tiles wide straddles every edge, half inside and half out. `roundness`
/// shapes the line's profile, from flat-topped (0) to a round bead (1, a profile
/// that bevels as a height). Wraps, so a tiling mask gives tiling lines; a mask
/// with no edge reads black.
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
        let half = Field::Const(self.width / 2.0);
        let inside = distance::distance(img, half);
        let outside = distance::distance(&complement(img), half);
        let round = self.roundness.clamp(0.0, 1.0);
        let mut out = Image::new(img.size());
        out.for_each_row(|y, row| {
            let (src, a, b) = (img.row(y), inside.row(y), outside.row(y));
            for (x, o) in row.iter_mut().enumerate() {
                let t = if src[x][0] > 0.5 { a[x][0] } else { b[x][0] };
                let v = profile(t, round);
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

/// The line's height at `t`, the distance from the edge over half the width
/// (`[0, 1]`, 1 past the line): 1 across the line when flat, `√(1 − t²)` when
/// round, and 0 past it either way.
fn profile(t: f32, roundness: f32) -> f32 {
    if t >= 1.0 {
        return 0.0;
    }
    let round = (1.0 - t * t).max(0.0).sqrt();
    1.0 + (round - 1.0) * roundness
}

#[cfg(test)]
#[path = "edge_detect_tests.rs"]
mod tests;
