//! The `clamp` op.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::math;
use crate::recipe::{op, req, OpInfo, ONE};

/// Clamp each channel into `[min, max]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clamp {
    pub min: f32,
    pub max: f32,
}

impl Row for Clamp {
    const INFO: OpInfo = op(
        "clamp",
        "math",
        ONE,
        &[req("min", "number"), req("max", "number")],
    );
}

impl Op for Clamp {
    fn run(&self, cx: &Cx) -> Image {
        math::clamp(cx.own(), self.min, self.max)
    }
}
