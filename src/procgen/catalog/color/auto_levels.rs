//! The `auto_levels` op: stretch RGB to fill `[0, 1]`.

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::color;
use crate::procgen::recipe::{op, OpInfo, ONE};

/// Stretch RGB so the input's minimum maps to 0 and its maximum to 1, one range
/// across R, G and B (hue kept); alpha untouched, a flat input unchanged.
pub struct AutoLevels;

impl Row for AutoLevels {
    const INFO: OpInfo = op("auto_levels", "color", ONE, &[]);
}

impl Op for AutoLevels {
    fn run(&self, cx: &Cx) -> Image {
        color::auto_levels(cx.own())
    }
}
