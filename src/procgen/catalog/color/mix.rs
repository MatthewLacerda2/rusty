//! The `mix` op: two inputs blended, through an optional mask.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::color;
use crate::procgen::recipe::{arity, choice, op, opt, BlendMode, Lit, OpInfo};

const BLEND: &[&str] = &[
    "mix",
    "add",
    "multiply",
    "screen",
    "overlay",
    "subtract",
    "difference",
    "lighten",
    "darken",
    "divide",
];

/// Blend inputs `a`, `b` by `factor` under a [`BlendMode`]; an optional third input
/// is a per-pixel mask whose red multiplies `factor` (#405).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    pub mode: BlendMode,
    #[serde(default = "one")]
    pub factor: f32,
}

impl Row for Mix {
    const INFO: OpInfo = op(
        "mix",
        "color",
        arity(2, 3),
        &[
            choice("mode", BLEND),
            opt("factor", "number", Lit::Number(1.0)),
        ],
    );
}

impl Op for Mix {
    fn run(&self, cx: &Cx) -> Image {
        color::mix(
            cx.own(),
            cx.input(1),
            cx.optional(2),
            self.mode,
            self.factor,
        )
    }
}

fn one() -> f32 {
    1.0
}
