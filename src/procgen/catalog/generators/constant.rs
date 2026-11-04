//! The `constant` op: a flat colour.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{color, op, req, OpInfo, SOURCE};

/// A flat color fill.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constant {
    #[serde(deserialize_with = "color::color")]
    pub color: [f32; 4],
}

impl Row for Constant {
    const INFO: OpInfo = op("constant", "generator", SOURCE, &[req("color", "color")]);
}

impl Op for Constant {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::constant(self))
    }
}
