//! The `wave` op: bands or rings.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{choice, op, req, OpInfo, WaveKind, SOURCE};

/// Bands or rings (grayscale), `frequency` cycles across the domain.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wave {
    pub kind: WaveKind,
    pub frequency: f32,
}

impl Row for Wave {
    const INFO: OpInfo = op(
        "wave",
        "generator",
        SOURCE,
        &[
            choice("kind", &["bands", "rings"]),
            req("frequency", "number"),
        ],
    );
}

impl Op for Wave {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::wave(self.kind, self.frequency))
    }
}
