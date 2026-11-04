//! The `rgb_to_bw` op.

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::math;
use crate::procgen::recipe::{op, OpInfo, ONE};

/// Collapse RGB to luminance (Rec. 709), written to all of RGB.
pub struct RgbToBw;

impl Row for RgbToBw {
    const INFO: OpInfo = op("rgb_to_bw", "math", ONE, &[]);
}

impl Op for RgbToBw {
    fn run(&self, cx: &Cx) -> Image {
        math::rgb_to_bw(cx.own())
    }
}
