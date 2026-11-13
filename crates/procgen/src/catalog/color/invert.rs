//! The `invert` op.

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::color;
use crate::recipe::{op, OpInfo, ONE};

/// Invert RGB (`1 - c`), alpha untouched.
pub struct Invert;

impl Row for Invert {
    const INFO: OpInfo = op("invert", "color", ONE, &[]);
}

impl Op for Invert {
    fn run(&self, cx: &Cx) -> Image {
        color::invert(cx.own())
    }
}
