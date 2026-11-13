//! The `bump_to_normal` op: a height field baked to a tangent-space normal map.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::vector;
use crate::recipe::{op, req, socket, OpInfo, Param, ONE};

/// Treat the input's red as a height field and bake a tangent-space normal map.
/// `strength` scales the slope measured in **tile units** (height change across one
/// whole tile), so it bakes the same normals at any resolution (#394).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BumpToNormal {
    pub strength: Param,
}

impl Row for BumpToNormal {
    const INFO: OpInfo = op(
        "bump_to_normal",
        "vector",
        ONE,
        &[socket(req("strength", "number"))],
    );
}

impl Op for BumpToNormal {
    fn run(&self, cx: &Cx) -> Image {
        vector::bump_to_normal(cx.input(0), cx.field(&self.strength))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("strength", &self.strength)]
    }
}
