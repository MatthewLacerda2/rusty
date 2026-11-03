//! The `tile` op: the seeded grid scatter (#407).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::tile;
use crate::procgen::recipe::{op, opt, req, socket, Lit, OpInfo, Param, ONE, SEED};

/// Scatter: stamp the input once per cell of a `count`×`count` grid, each cell
/// seeded with an offset (`jitter`, in cells), a rotation (`rotation_jitter`, in
/// turns) and a scale (`scale_jitter`); overlaps keep the brighter stamp. Always
/// tiles. A linked jitter is read once per cell, at its centre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub count: f32,
    #[serde(default)]
    pub jitter: Param,
    #[serde(default)]
    pub rotation_jitter: Param,
    #[serde(default)]
    pub scale_jitter: Param,
    #[serde(default)]
    pub seed: u32,
}

impl Row for Tile {
    const INFO: OpInfo = op(
        "tile",
        "vector",
        ONE,
        &[
            req("count", "number"),
            socket(opt("jitter", "number", Lit::Number(0.0))),
            socket(opt("rotation_jitter", "number", Lit::Number(0.0))),
            socket(opt("scale_jitter", "number", Lit::Number(0.0))),
            SEED,
        ],
    );
}

impl Op for Tile {
    fn run(&self, cx: &Cx) -> Image {
        let jitters = tile::Jitters::of(self, cx.links);
        tile::tile(cx.input(0), self.count, jitters, cx.seed)
    }

    fn seed(&self) -> u32 {
        self.seed
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![
            ("jitter", &self.jitter),
            ("rotation_jitter", &self.rotation_jitter),
            ("scale_jitter", &self.scale_jitter),
        ]
    }
}
