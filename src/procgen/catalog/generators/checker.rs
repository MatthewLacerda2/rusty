//! The `checker` op: a two-colour checkerboard.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{color, op, req, OpInfo, SOURCE};

/// A 2-color checkerboard; `tiles` squares across each axis (rounded up to even).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checker {
    pub tiles: u32,
    #[serde(deserialize_with = "color::color_a")]
    pub color_a: [f32; 4],
    #[serde(deserialize_with = "color::color_b")]
    pub color_b: [f32; 4],
}

impl Row for Checker {
    const INFO: OpInfo = op(
        "checker",
        "generator",
        SOURCE,
        &[
            req("tiles", "integer"),
            req("color_a", "color"),
            req("color_b", "color"),
        ],
    );
}

impl Op for Checker {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::checker(self))
    }
}
