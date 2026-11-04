//! The `sharpen` op: an unsharp mask (#1743).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::filter;
use crate::procgen::recipe::{op, opt, BlurKind, Lit, OpInfo, ONE};

/// `c + intensity × (c − blur(c))` per RGB channel, with `blur`'s Gaussian at
/// `radius` (a tile fraction, so it crisps the same detail at any resolution).
/// Wraps, so it tiles; unclamped, as every op is until the bake. Alpha untouched.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sharpen {
    #[serde(default = "one")]
    pub intensity: f32,
    #[serde(default = "radius")]
    pub radius: f32,
}

impl Row for Sharpen {
    const INFO: OpInfo = op(
        "sharpen",
        "filter",
        ONE,
        &[
            opt("intensity", "number", Lit::Number(1.0)),
            opt("radius", "number", Lit::Number(RADIUS)),
        ],
    );
}

/// The default reach: two pixels at 512², eight at 2048².
const RADIUS: f32 = 0.004;

impl Op for Sharpen {
    fn run(&self, cx: &Cx) -> Image {
        let img = cx.input(0);
        let k = self.intensity;
        // The blur is a private buffer: the result is written into it.
        let mut out = filter::blur(img, cx.px(self.radius), BlurKind::Gaussian);
        out.for_each_row(|y, row| {
            for (b, p) in row.iter_mut().zip(img.row(y)) {
                let f = |c: usize| p[c] + k * (p[c] - b[c]);
                *b = [f(0), f(1), f(2), p[3]];
            }
        });
        out
    }
}

fn one() -> f32 {
    1.0
}

fn radius() -> f32 {
    RADIUS
}

#[cfg(test)]
#[path = "sharpen_tests.rs"]
mod tests;
