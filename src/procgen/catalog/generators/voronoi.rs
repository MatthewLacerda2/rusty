//! The `voronoi` op: Voronoi / Worley cellular noise.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{
    choice, op, opt, req, socket, Lit, OpInfo, OpParam, Param, VoronoiMetric, VoronoiOutput, SEED,
    SOURCE,
};

/// Voronoi / Worley cellular pattern; `randomness` jitters each cell's point (0 = a
/// regular grid, 1 = anywhere in its cell); `metric` is how distance is measured.
/// A linked `randomness` is read once per cell, at its centre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Voronoi {
    pub scale: f32,
    #[serde(default)]
    pub output: VoronoiOutput,
    #[serde(default)]
    pub metric: VoronoiMetric,
    #[serde(default = "one")]
    pub randomness: Param,
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
            OpParam {
                default: Some(Lit::Text("distance")),
                ..choice("output", &["distance", "cells", "f2", "edges"])
            },
            OpParam {
                default: Some(Lit::Text("euclidean")),
                ..choice("metric", &["euclidean", "manhattan", "chebyshev"])
            },
            socket(opt("randomness", "number", Lit::Number(1.0))),
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
