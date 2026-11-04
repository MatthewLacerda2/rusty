//! The `shape` op: one centred shape per tile, as a mask.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{choice, op, opt, req, socket, Lit, OpInfo, Param, ShapeKind, SOURCE};

/// One centred shape per tile as a mask (1 inside, 0 outside). `size` is its
/// width/height in tile units; `roundness` (0–1) rounds a `rounded_rect`'s corners;
/// `softness` (tile units) ramps the edge inward, a bevel profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    pub size: [f32; 2],
    #[serde(default = "quarter")]
    pub roundness: f32,
    #[serde(default)]
    pub softness: Param,
}

impl Row for Shape {
    const INFO: OpInfo = op(
        "shape",
        "generator",
        SOURCE,
        &[
            choice("kind", &["circle", "rect", "rounded_rect", "line"]),
            req("size", "vec2"),
            opt("roundness", "number", Lit::Number(0.25)),
            socket(opt("softness", "number", Lit::Number(0.0))),
        ],
    );
}

impl Op for Shape {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::shape(self, cx.field(&self.softness)))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("softness", &self.softness)]
    }
}

fn quarter() -> f32 {
    0.25
}
