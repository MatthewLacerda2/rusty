//! The `scatter` op (#2187): stamps placed freely over the tile, off any grid,
//! Substance Designer's *Shape Splatter* and the free half of its *Tile Sampler*.

use serde::{Deserialize, Serialize};

use crate::catalog::vector::tile::{TileBlend, TileOutput};
use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::scatter;
use crate::recipe::{arity, choice, op, opt, socket, Lit, OpInfo, OpParam, Param, SEED};

/// Free scatter: `amount` stamps (a whole count) at hashed points over the torus,
/// each one of the 1–8 stamp inputs picked by hash, `scale` tiles across, turned
/// `rotation` turns, each jittered, and drawn with the chance `density`. `density`,
/// `scale` and `rotation` link, read once per stamp at its centre. `spacing` (0–1)
/// keeps the stamps apart. `output` and `blend` mean what `tile`'s do; under `over`
/// the stamps are laid in their index order. Always tiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scatter {
    #[serde(default = "amount")]
    pub amount: f32,
    #[serde(default = "scale")]
    pub scale: Param,
    #[serde(default)]
    pub scale_jitter: f32,
    #[serde(default)]
    pub rotation: Param,
    #[serde(default)]
    pub rotation_jitter: f32,
    #[serde(default)]
    pub value_jitter: f32,
    #[serde(default)]
    pub spacing: f32,
    #[serde(default = "always")]
    pub density: Param,
    #[serde(default)]
    pub output: TileOutput,
    #[serde(default)]
    pub blend: TileBlend,
    #[serde(default)]
    pub seed: u32,
}

/// `amount`'s default: 32 stamps per tile.
fn amount() -> f32 {
    32.0
}

/// `scale`'s default: a stamp a fifth of the tile across.
fn scale() -> Param {
    Param::Num(0.2)
}

/// `density`'s default: every stamp is drawn.
fn always() -> Param {
    Param::Num(1.0)
}

impl Scatter {
    /// `amount` stamps with every other param at its default.
    pub fn new(amount: f32) -> Self {
        Self {
            amount,
            scale: scale(),
            scale_jitter: 0.0,
            rotation: Param::default(),
            rotation_jitter: 0.0,
            value_jitter: 0.0,
            spacing: 0.0,
            density: always(),
            output: TileOutput::default(),
            blend: TileBlend::default(),
            seed: 0,
        }
    }
}

impl Row for Scatter {
    const INFO: OpInfo = op(
        "scatter",
        "vector",
        arity(1, 8),
        &[
            opt("amount", "number", Lit::Number(32.0)),
            socket(opt("scale", "number", Lit::Number(0.2))),
            opt("scale_jitter", "number", Lit::Number(0.0)),
            socket(opt("rotation", "number", Lit::Number(0.0))),
            opt("rotation_jitter", "number", Lit::Number(0.0)),
            opt("value_jitter", "number", Lit::Number(0.0)),
            opt("spacing", "number", Lit::Number(0.0)),
            socket(opt("density", "number", Lit::Number(1.0))),
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

impl Op for Scatter {
    fn run(&self, cx: &Cx) -> Image {
        let s = scatter::Settings::of(self, cx.links, cx.seed);
        scatter::scatter(cx.inputs, &s)
    }

    fn seed(&self) -> u32 {
        self.seed
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![
            ("scale", &self.scale),
            ("rotation", &self.rotation),
            ("density", &self.density),
        ]
    }
}
