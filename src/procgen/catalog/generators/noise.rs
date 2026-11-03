//! The `noise` op: Perlin and its fBM / ridged / turbulence sums.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{
    choice, op, opt, req, socket, Lit, NoiseKind, OpInfo, Param, SEED, SOURCE,
};

/// Gradient noise (grayscale, written to RGB, alpha 1). The fBM family (`fbm`,
/// `ridged`, `turbulence`) sums `octaves`, each `lacunarity`× the previous frequency
/// (rounded to a whole count, so every octave tiles) and `gain`× its amplitude;
/// `perlin` is one octave.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Noise {
    pub kind: NoiseKind,
    pub scale: f32,
    #[serde(default = "one")]
    pub octaves: u32,
    #[serde(default = "two")]
    pub lacunarity: f32,
    #[serde(default = "half")]
    pub gain: Param,
    #[serde(default)]
    pub seed: u32,
}

impl Row for Noise {
    const INFO: OpInfo = op(
        "noise",
        "generator",
        SOURCE,
        &[
            choice("kind", &["perlin", "fbm", "ridged", "turbulence"]),
            req("scale", "number"),
            opt("octaves", "integer", Lit::Integer(1)),
            opt("lacunarity", "number", Lit::Number(2.0)),
            socket(opt("gain", "number", Lit::Number(0.5))),
            SEED,
        ],
    );
}

impl Op for Noise {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::noise(self, cx.field(&self.gain), cx.seed))
    }

    fn seed(&self) -> u32 {
        self.seed
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("gain", &self.gain)]
    }
}

fn one() -> u32 {
    1
}

fn two() -> f32 {
    2.0
}

fn half() -> Param {
    Param::Num(0.5)
}
