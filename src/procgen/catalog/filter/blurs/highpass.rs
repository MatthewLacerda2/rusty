//! The `highpass` op: an image's fine detail without its large shapes (Substance
//! Designer's *Highpass*, #1743).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::filter;
use crate::procgen::recipe::{op, req, BlurKind, OpInfo, ONE};

/// `c − blur(c) + 0.5` per RGB channel, with `blur`'s Gaussian at `radius` (a tile
/// fraction): detail smaller than the radius kept around mid-grey, larger shapes
/// flattened. Wraps, so it tiles. Alpha untouched; a radius of 0 reads flat 0.5.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Highpass {
    pub radius: f32,
}

impl Row for Highpass {
    const INFO: OpInfo = op("highpass", "filter", ONE, &[req("radius", "number")]);
}

impl Op for Highpass {
    fn run(&self, cx: &Cx) -> Image {
        let img = cx.input(0);
        // The blur is a private buffer: the result is written into it.
        let mut out = filter::blur(img, cx.px(self.radius), BlurKind::Gaussian);
        out.for_each_row(|y, row| {
            for (b, p) in row.iter_mut().zip(img.row(y)) {
                *b = [
                    p[0] - b[0] + 0.5,
                    p[1] - b[1] + 0.5,
                    p[2] - b[2] + 0.5,
                    p[3],
                ];
            }
        });
        out
    }
}

#[cfg(test)]
#[path = "highpass_tests.rs"]
mod tests;
