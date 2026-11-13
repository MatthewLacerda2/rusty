//! The `curvature` op: a height field's convexity (#408).

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::filter;
use crate::recipe::{op, opt, req, socket, Lit, OpInfo, Param, ONE};

/// Height → curvature: `0.5 + strength × (h − mean of h on a ring of `radius`)` (tile
/// units). Above 0.5 convex (edge wear), below concave (grime) (#408).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curvature {
    pub radius: f32,
    #[serde(default = "one")]
    pub strength: Param,
}

impl Row for Curvature {
    const INFO: OpInfo = op(
        "curvature",
        "filter",
        ONE,
        &[
            req("radius", "number"),
            socket(opt("strength", "number", Lit::Number(1.0))),
        ],
    );
}

impl Op for Curvature {
    fn run(&self, cx: &Cx) -> Image {
        filter::curvature(cx.input(0), self.radius, cx.field(&self.strength))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("strength", &self.strength)]
    }
}

fn one() -> Param {
    Param::Num(1.0)
}
