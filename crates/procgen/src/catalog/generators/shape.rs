use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators;
use crate::recipe::{choice, op, opt, req, socket, Lit, OpInfo, OpParam, Param, ShapeKind, SOURCE};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeProfile {
    #[default]
    Flat,
    Cone,
    Paraboloid,
    Hemisphere,
}

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
