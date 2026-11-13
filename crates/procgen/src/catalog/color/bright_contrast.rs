//! The `bright_contrast` op.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::color;
use crate::recipe::{op, req, socket, OpInfo, Param, ONE};

/// Brightness offset then contrast about 0.5.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrightContrast {
    pub bright: Param,
    pub contrast: Param,
}

impl Row for BrightContrast {
    const INFO: OpInfo = op(
        "bright_contrast",
        "color",
        ONE,
        &[
            socket(req("bright", "number")),
            socket(req("contrast", "number")),
        ],
    );
}

impl Op for BrightContrast {
    fn run(&self, cx: &Cx) -> Image {
        color::bright_contrast(cx.own(), cx.field(&self.bright), cx.field(&self.contrast))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("bright", &self.bright), ("contrast", &self.contrast)]
    }
}
