//! The `anisotropic_noise` op: long thin fibres at an angle (#1740).

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators;
use crate::recipe::{op, opt, req, Lit, OpInfo, SEED, SOURCE};

/// Value noise stretched into fibres, Substance Designer's *Anisotropic Noise*:
/// about `scale` fibres across the tile, each `anisotropy` times longer than wide,
/// running at `angle` turns (0 along `u`), snapped to the nearest direction that
/// tiles. Brushed metal, wood grain, fabric, scratches.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnisotropicNoise {
    pub scale: f32,
    #[serde(default = "eight")]
    pub anisotropy: f32,
    #[serde(default)]
    pub angle: f32,
    #[serde(default)]
    pub seed: u32,
}

impl Row for AnisotropicNoise {
    const INFO: OpInfo = op(
        "anisotropic_noise",
        "generator",
        SOURCE,
        &[
            req("scale", "number"),
            opt("anisotropy", "number", Lit::Number(8.0)),
            opt("angle", "number", Lit::Number(0.0)),
            SEED,
        ],
    );
}

impl Op for AnisotropicNoise {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::anisotropic::sampler(self, cx.seed))
    }

    fn seed(&self) -> u32 {
        self.seed
    }
}

fn eight() -> f32 {
    8.0
}
