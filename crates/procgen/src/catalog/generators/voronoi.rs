use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators;
use crate::recipe::{
    choice, op, opt, req, socket, Lit, OpInfo, OpParam, Param, VoronoiMetric, VoronoiOutput, SEED,
    SOURCE,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Voronoi {
    pub scale: f32,
    #[serde(default = "square")]
    pub aspect: [f32; 2],
    #[serde(default)]
    pub output: VoronoiOutput,
    #[serde(default)]
    pub metric: VoronoiMetric,
    #[serde(default = "one")]
    pub randomness: Param,
    #[serde(default = "one_f32")]
    pub smoothness: f32,
    #[serde(default)]
    pub grayscale: bool,
    #[serde(default)]
    pub seed: u32,
}

impl Row for Voronoi {
    const INFO: OpInfo = op(
        "voronoi",
        "generator",
        SOURCE,
        &[
            req("scale", "number"),
            opt("aspect", "vec2", Lit::Vec2([1.0, 1.0])),
            OpParam {
                default: Some(Lit::Text("distance")),
                ..choice("output", &["distance", "cells", "f2", "edges", "smooth_f1"])
            },
            OpParam {
                default: Some(Lit::Text("euclidean")),
                ..choice("metric", &["euclidean", "manhattan", "chebyshev"])
            },
            socket(opt("randomness", "number", Lit::Number(1.0))),
            opt("smoothness", "number", Lit::Number(1.0)),
            opt("grayscale", "bool", Lit::Bool(false)),
            SEED,
        ],
    );
}

impl Op for Voronoi {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::voronoi(
            self,
            cx.field(&self.randomness),
            cx.seed,
        ))
    }

    fn seed(&self) -> u32 {
        self.seed
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("randomness", &self.randomness)]
    }
}

fn one() -> Param {
    Param::Num(1.0)
}

fn one_f32() -> f32 {
    1.0
}

fn square() -> [f32; 2] {
    [1.0, 1.0]
}
