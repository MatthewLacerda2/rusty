//! The `combine_rgb` op.

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::vector;
use crate::procgen::recipe::{arity, op, OpInfo};

/// Combine up to three single-channel inputs into one RGB image (alpha 1); a
/// channel without an input reads 0.
pub struct CombineRgb;

impl Row for CombineRgb {
    const INFO: OpInfo = op("combine_rgb", "vector", arity(1, 3), &[]);
}

impl Op for CombineRgb {
    fn run(&self, cx: &Cx) -> Image {
        vector::combine_rgb(cx.inputs, cx.size)
    }
}
