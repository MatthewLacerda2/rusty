//! The `mapping` op: an affine transform of the input's domain.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::vector;
use crate::recipe::{op, opt, Lit, OpInfo, ONE};

/// Scale / rotate (turns) / translate the *domain* of the single input, resampled
/// bilinearly with wrap. With `tiling` (the default) `scale` rounds to whole repeats
/// and `rotation` snaps to quarter turns, so the output tiles; `tiling = false`
/// honours free values for decals and one-off maps, at the cost of a seam. Each
/// transform left out is the identity, as in Blender's Mapping node (#1575).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    #[serde(default = "one_vec2")]
    pub scale: [f32; 2],
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub translation: [f32; 2],
    #[serde(default = "yes")]
    pub tiling: bool,
}

impl Row for Mapping {
    const INFO: OpInfo = op(
        "mapping",
        "vector",
        ONE,
        &[
            opt("scale", "vec2", Lit::Vec2([1.0, 1.0])),
            opt("rotation", "number", Lit::Number(0.0)),
            opt("translation", "vec2", Lit::Vec2([0.0, 0.0])),
            opt("tiling", "bool", Lit::Bool(true)),
        ],
    );
}

impl Op for Mapping {
    fn run(&self, cx: &Cx) -> Image {
        let p = self;
        vector::mapping(cx.input(0), p.scale, p.rotation, p.translation, p.tiling)
    }
}

fn one_vec2() -> [f32; 2] {
    [1.0, 1.0]
}

fn yes() -> bool {
    true
}
