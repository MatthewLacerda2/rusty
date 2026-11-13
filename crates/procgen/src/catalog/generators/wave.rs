//! The `wave` op: bands or rings, with Blender's Wave Texture profiles and
//! distortion.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators;
use crate::recipe::{choice, op, opt, req, Lit, OpInfo, OpParam, WaveKind, SEED, SOURCE};

/// The shape of one cycle, Blender's Wave Texture *profile*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveProfile {
    /// `0.5 + 0.5 sin`: soft, round crests.
    #[default]
    Sine,
    /// A ramp 0 → 1, then a drop: planks, shingles, terraces.
    Saw,
    /// 0 → 1 → 0 in straight lines: corrugation, ridges.
    Triangle,
}

/// Bands or rings (grayscale), `frequency` cycles across the domain, each cycle
/// shaped by `profile`. `distortion` pushes the phase by a tiling fBM of `detail + 1`
/// octaves at the wave's own frequency, in radians as Blender's: wood rings and
/// marble without a separate warp chain.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wave {
    pub kind: WaveKind,
    pub frequency: f32,
    #[serde(default)]
    pub profile: WaveProfile,
    #[serde(default)]
    pub distortion: f32,
    #[serde(default = "two")]
    pub detail: u32,
    #[serde(default)]
    pub seed: u32,
}

impl Row for Wave {
    const INFO: OpInfo = op(
        "wave",
        "generator",
        SOURCE,
        &[
            choice("kind", &["bands", "rings"]),
            req("frequency", "number"),
            OpParam {
                default: Some(Lit::Text("sine")),
                ..choice("profile", &["sine", "saw", "triangle"])
            },
            opt("distortion", "number", Lit::Number(0.0)),
            opt("detail", "integer", Lit::Integer(2)),
            SEED,
        ],
    );
}

impl Op for Wave {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::wave(self, cx.seed))
    }

    fn seed(&self) -> u32 {
        self.seed
    }
}

fn two() -> u32 {
    2
}
