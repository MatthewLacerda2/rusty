//! The `map_range` op.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::math;
use crate::recipe::{op, req, OpInfo, ONE};

/// Remap each channel from `[from_min, from_max]` to `[to_min, to_max]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapRange {
    pub from_min: f32,
    pub from_max: f32,
    pub to_min: f32,
    pub to_max: f32,
}

impl Row for MapRange {
    const INFO: OpInfo = op(
        "map_range",
        "math",
        ONE,
        &[
            req("from_min", "number"),
            req("from_max", "number"),
            req("to_min", "number"),
            req("to_max", "number"),
        ],
    );
}

impl Op for MapRange {
    fn run(&self, cx: &Cx) -> Image {
        let p = self;
        math::map_range(cx.own(), p.from_min, p.from_max, p.to_min, p.to_max)
    }
}
