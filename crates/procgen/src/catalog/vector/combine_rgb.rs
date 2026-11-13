//! The `combine_rgb` op.

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::vector;
use crate::recipe::{arity, op, OpInfo};

/// Combine up to four single-channel inputs into one image, R, G, B then alpha
/// (#2002); a colour channel without an input reads 0, alpha without one reads 1.
pub struct CombineRgb;

impl Row for CombineRgb {
    const INFO: OpInfo = op("combine_rgb", "vector", arity(1, 4), &[]);
}

impl Op for CombineRgb {
    fn run(&self, cx: &Cx) -> Image {
        vector::combine_rgb(cx.inputs, cx.size)
    }
}
