use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators;
use crate::recipe::{op, opt, req, Lit, OpInfo, SEED, SOURCE};

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
