//! The `warp` op: a domain warp.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::vector;
use crate::procgen::recipe::{arity, op, req, socket, OpInfo, Param};

/// Domain warp: sample input A displaced by input B's R/G, centred on 0.5 and scaled
/// by `strength` in **tile units**; wrapped, so it tiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    pub strength: Param,
}

impl Row for Warp {
    const INFO: OpInfo = op(
        "warp",
        "vector",
        arity(2, 2),
        &[socket(req("strength", "number"))],
    );
}

impl Op for Warp {
    fn run(&self, cx: &Cx) -> Image {
        vector::warp(cx.input(0), cx.input(1), cx.field(&self.strength))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("strength", &self.strength)]
    }
}
