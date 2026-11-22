use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::filter;
use crate::recipe::{op, req, BlurKind, OpInfo, ONE};

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
