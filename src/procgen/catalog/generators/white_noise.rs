//! The `white_noise` op: per-pixel uniform noise.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{op, OpInfo, SEED, SOURCE};

/// Per-pixel uniform white noise (grayscale).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WhiteNoise {
    #[serde(default)]
    pub seed: u32,
}

impl Row for WhiteNoise {
    const INFO: OpInfo = op("white_noise", "generator", SOURCE, &[SEED]);
}

impl Op for WhiteNoise {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::white_noise(cx.size, cx.seed))
    }

    fn seed(&self) -> u32 {
        self.seed
    }
}
