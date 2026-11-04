//! The `invert` op.

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::color;
use crate::procgen::recipe::{op, OpInfo, ONE};

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
