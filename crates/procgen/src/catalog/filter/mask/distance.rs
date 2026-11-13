//! The `distance` op: a mask's distance from its edge, a bevel (#1492).

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::distance;
use crate::recipe::{op, req, socket, OpInfo, Param, ONE};

/// Mask → bevel (#1492): red > 0.5 is inside; each inside pixel reads its distance
/// to the mask's edge over `radius` (tile units), clamped to 1. Outside reads 0.
/// Wraps, so a tiling mask gives a tiling bevel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Distance {
    pub radius: Param,
}

impl Row for Distance {
    const INFO: OpInfo = op(
        "distance",
        "filter",
        ONE,
        &[socket(req("radius", "number"))],
    );
}

impl Op for Distance {
    fn run(&self, cx: &Cx) -> Image {
        distance::distance(cx.input(0), cx.field(&self.radius))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("radius", &self.radius)]
    }
}
