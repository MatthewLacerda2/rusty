//! The `separate_rgb` op.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::vector;
use crate::procgen::recipe::{op, req, OpInfo, ONE};

/// Pull one channel of the input out as grayscale. `channel` 0=R,1=G,2=B,3=A.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeparateRgb {
    pub channel: u8,
}

impl Row for SeparateRgb {
    const INFO: OpInfo = op("separate_rgb", "vector", ONE, &[req("channel", "integer")]);
}

impl Op for SeparateRgb {
    fn run(&self, cx: &Cx) -> Image {
        vector::separate_rgb(cx.own(), self.channel)
    }
}
