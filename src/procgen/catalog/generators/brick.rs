//! The `brick` op: a running-bond pattern.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{
    choice, op, opt, req, BrickOutput, Lit, OpInfo, OpParam, SEED, SOURCE,
};

/// A brick / running-bond pattern; `rows`/`cols` bricks across the domain (`rows`
/// rounds to an even count so the running bond closes at the wrap). `output` picks
/// the face mask, a seeded value per brick, or a bevel ramp.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brick {
    pub rows: f32,
    pub cols: f32,
    #[serde(default = "default_mortar")]
    pub mortar: f32,
    #[serde(default)]
    pub output: BrickOutput,
    #[serde(default)]
    pub seed: u32,
}

impl Row for Brick {
    const INFO: OpInfo = op(
        "brick",
        "generator",
        SOURCE,
        &[
            req("rows", "number"),
            req("cols", "number"),
            opt("mortar", "number", Lit::Number(0.05)),
            OpParam {
                default: Some(Lit::Text("mask")),
                ..choice("output", &["mask", "random", "bevel"])
            },
            SEED,
        ],
    );
}

impl Op for Brick {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::brick(self, cx.seed))
    }

    fn seed(&self) -> u32 {
        self.seed
    }
}

fn default_mortar() -> f32 {
    0.05
}
