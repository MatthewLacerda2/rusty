//! The `flood_fill` op: a mask's islands (Substance Designer's *Flood Fill*, #1741).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::flood;
use crate::procgen::recipe::{op, opt, Lit, OpInfo, ONE};

/// Mask → its islands: the pixels whose red is above `threshold` (NaN is off),
/// joined through their 8 neighbours and wrapping both axes, so a piece across a
/// tile edge or corner is one island. Writes alpha 1 on an island and 0 off it,
/// which `flood_random`, `flood_gradient` and `flood_bbox` read; RGB is a colour
/// per island for the preview (black off), not a value to compute with.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloodFill {
    #[serde(default = "half")]
    pub threshold: f32,
}

fn half() -> f32 {
    0.5
}

impl Row for FloodFill {
    const INFO: OpInfo = op(
        "flood_fill",
        "filter",
        ONE,
        &[opt("threshold", "number", Lit::Number(0.5))],
    );
}

impl Op for FloodFill {
    fn run(&self, cx: &Cx) -> Image {
        let islands = flood::label(cx.input(0), |p| p[0] > self.threshold);
        let tint = |i: &flood::Island, c| 0.2 + 0.8 * i.random(c, 0);
        flood::paint_rgba(&islands, [0.0; 4], |i, _, _| {
            [tint(i, 0), tint(i, 1), tint(i, 2), 1.0]
        })
    }
}
