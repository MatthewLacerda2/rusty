//! The `gradient` op: a linear or radial ramp.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators;
use crate::recipe::{choice, op, GradientKind, OpInfo, SOURCE};

/// Linear or radial gradient ramp (grayscale).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub kind: GradientKind,
}

impl Row for Gradient {
    const INFO: OpInfo = op(
        "gradient",
        "generator",
        SOURCE,
        &[choice("kind", &["linear", "linear_tiling", "radial"])],
    );
}

impl Op for Gradient {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(generators::gradient(self.kind))
    }
}
