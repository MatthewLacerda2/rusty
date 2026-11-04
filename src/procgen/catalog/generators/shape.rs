//! The `shape` op: one centred shape per tile, as a mask or a height stamp.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{
    choice, op, opt, req, socket, Lit, OpInfo, OpParam, Param, ShapeKind, SOURCE,
};

/// What a shape reads inside, from its edge (0) to its deepest point (1): Substance
/// Designer's *Shape* heights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeProfile {
    /// 1 inside (`softness` ramps the edge): a mask.
    #[default]
    Flat,
    /// A straight slope to the middle: a cone on a circle, a pyramid on a square
    /// (a hip roof on a longer rect).
    Cone,
    /// `1 − (1 − t)²`: round on top, steep at the rim.
    Paraboloid,
    /// `√(1 − (1 − t)²)`: a dome, vertical at the rim (SD's bell is its softer kin).
    Hemisphere,
}

/// One centred shape per tile (1 inside, 0 outside). `size` is its width/height in
/// tile units; `roundness` (0–1) rounds a `rounded_rect`'s corners; `softness` (tile
/// units) ramps a `flat` edge inward, a bevel profile. `sides` is a `polygon`'s,
/// `points` and `inner_radius` (of the outer) a `star`'s. A `profile` other than
/// `flat` reads a height instead, 0 at the edge and 1 at the deepest point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    pub size: [f32; 2],
    #[serde(default = "quarter")]
    pub roundness: f32,
    #[serde(default)]
    pub softness: Param,
    #[serde(default = "six")]
    pub sides: u32,
    #[serde(default = "five")]
    pub points: u32,
    #[serde(default = "half")]
    pub inner_radius: f32,
    #[serde(default)]
    pub profile: ShapeProfile,
}

impl Row for Shape {
    const INFO: OpInfo = op(
        "shape",
        "generator",
        SOURCE,
        &[
            choice(
                "kind",
                &["circle", "rect", "rounded_rect", "line", "polygon", "star"],
            ),
            req("size", "vec2"),
            opt("roundness", "number", Lit::Number(0.25)),
            socket(opt("softness", "number", Lit::Number(0.0))),
            opt("sides", "integer", Lit::Integer(6)),
            opt("points", "integer", Lit::Integer(5)),
            opt("inner_radius", "number", Lit::Number(0.5)),
            OpParam {
                default: Some(Lit::Text("flat")),
                ..choice("profile", &["flat", "cone", "paraboloid", "hemisphere"])
            },
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

fn half() -> f32 {
    0.5
}

fn five() -> u32 {
    5
}

fn six() -> u32 {
    6
}
