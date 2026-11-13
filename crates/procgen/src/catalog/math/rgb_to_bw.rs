//! The `rgb_to_bw` op.

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::math;
use crate::recipe::{op, OpInfo, ONE};

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
