//! The `blur` op: a separable box or Gaussian blur.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::filter;
use crate::recipe::{choice, op, req, socket, BlurKind, Lit, OpInfo, OpParam, Param, ONE};

/// A separable blur (wraps at the edges), Gaussian by default. `radius` is the
/// kernel's reach as a **fraction of the tile width** (`0.01` = 1%), so it looks the
/// same at any resolution (#394). A linked radius varies per pixel (#1494).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blur {
    pub radius: Param,
    #[serde(default)]
    pub kind: BlurKind,
}

impl Row for Blur {
    const INFO: OpInfo = op(
        "blur",
        "filter",
        ONE,
        &[
            socket(req("radius", "number")),
            OpParam {
                default: Some(Lit::Text("gaussian")),
                ..choice("kind", &["box", "gaussian"])
            },
        ],
    );
}

impl Op for Blur {
    fn run(&self, cx: &Cx) -> Image {
        filter::blur_varying(cx.input(0), cx.field(&self.radius), self.kind)
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("radius", &self.radius)]
    }
}
