//! The `tile` op: the seeded grid scatter (#407), grown toward Substance Designer's
//! *Tile Sampler* (#1742): a value per stamp, fill probability, a mask, a size map
//! and an overlap blend.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::tile;
use crate::recipe::{arity, choice, op, opt, req, socket, Lit, OpInfo, OpParam, Param, SEED};

/// What each stamp is filled with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileOutput {
    /// The input's own colour.
    #[default]
    Stamp,
    /// The input's red times a seeded grey per stamp, in `[0, 1)`.
    Random,
    /// The input's red times the cell's row-major index `(i + 1) / count²`.
    Index,
}

/// How overlapping stamps combine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileBlend {
    /// The brighter value per channel.
    #[default]
    Max,
    /// The colours summed (alpha keeps the larger).
    Add,
    /// Each stamp laid over the ones before it in row-major cell order,
    /// premultiplied, its brightness (times its alpha) read as its coverage.
    Over,
}

/// Scatter: stamp the input once per cell of a `count`×`count` grid, each cell
/// seeded with an offset (`jitter`, in cells), a rotation (`rotation_jitter`, in
/// turns), a scale (`scale_jitter`), a value (`value_jitter`) and a chance of
/// being stamped at all (`probability`). Optional input 2 is a mask (no stamp
/// where its red reads below 0.5), input 3 a size map (its red scales the stamp);
/// both are read at the cell's centre. Always tiles.
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

/// `probability`'s default: every cell gets a stamp.
fn always() -> f32 {
    1.0
}

impl Tile {
    /// A `count`×`count` scatter with every other param at its default.
    pub fn new(count: f32) -> Self {
        Self {
            count,
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
        let scatter = tile::Scatter::of(self, cx.links, cx.optional(1), cx.optional(2), cx.seed);
        tile::tile(cx.input(0), &scatter)
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
