//! The `tile` op: the seeded grid scatter, after Substance Designer's *Tile Sampler*.
use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::tile;
use crate::recipe::{arity, choice, op, opt, req, socket, Lit, OpInfo, OpParam, Param, SEED};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileOutput {
    #[default]
    Stamp,
    Random,
    Index,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileBlend {
    #[default]
    Max,
    Add,
    Over,
}

/// Grid stamping: the input once per cell of a `count × aspect` grid, each cell seeded with
/// jitters (`jitter` in cells, `rotation_jitter` in turns) and a chance of being stamped
/// (`probability`). Optional input 2 masks (no stamp where its red is below 0.5), input 3 is a
/// size map; both read at the cell's centre. Always tiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub count: f32,
    #[serde(default = "square")]
    pub aspect: [f32; 2],
    /// Every other row shifted by this share of a cell (0–1); 0.5 is a running bond.
    #[serde(default)]
    pub offset: f32,
    #[serde(default)]
    pub jitter: Param,
    #[serde(default)]
    pub rotation_jitter: Param,
    #[serde(default)]
    pub scale_jitter: Param,
    #[serde(default)]
    pub value_jitter: f32,
    #[serde(default = "always")]
    pub probability: f32,
    #[serde(default)]
    pub output: TileOutput,
    #[serde(default)]
    pub blend: TileBlend,
    #[serde(default)]
    pub seed: u32,
}

fn square() -> [f32; 2] {
    [1.0, 1.0]
}

fn always() -> f32 {
    1.0
}

impl Tile {
    pub fn new(count: f32) -> Self {
        Self {
            count,
            aspect: square(),
            offset: 0.0,
            jitter: Param::default(),
            rotation_jitter: Param::default(),
            scale_jitter: Param::default(),
            value_jitter: 0.0,
            probability: always(),
            output: TileOutput::default(),
            blend: TileBlend::default(),
            seed: 0,
        }
    }
}

impl Row for Tile {
    const INFO: OpInfo = op(
        "tile",
        "vector",
        arity(1, 3),
        &[
            req("count", "number"),
            opt("aspect", "vec2", Lit::Vec2([1.0, 1.0])),
            opt("offset", "number", Lit::Number(0.0)),
            socket(opt("jitter", "number", Lit::Number(0.0))),
            socket(opt("rotation_jitter", "number", Lit::Number(0.0))),
            socket(opt("scale_jitter", "number", Lit::Number(0.0))),
            opt("value_jitter", "number", Lit::Number(0.0)),
            opt("probability", "number", Lit::Number(1.0)),
            OpParam {
                default: Some(Lit::Text("stamp")),
                ..choice("output", &["stamp", "random", "index"])
            },
            OpParam {
                default: Some(Lit::Text("max")),
                ..choice("blend", &["max", "add", "over"])
            },
            SEED,
        ],
    );
}

impl Op for Tile {
    fn run(&self, cx: &Cx) -> Image {
        let s = tile::Settings::of(self, cx.links, cx.optional(1), cx.optional(2), cx.seed);
        tile::tile(cx.input(0), &s)
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
