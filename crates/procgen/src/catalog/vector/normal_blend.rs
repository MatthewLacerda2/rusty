//! The `normal_blend` op: Reoriented Normal Mapping.
use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::vector;
use crate::recipe::{arity, op, opt, socket, Lit, OpInfo, Param};

/// Reoriented Normal Mapping: lay the second input's normal map (detail)
/// over the first's (base). `strength` scales the detail's tilt first; 0 leaves the
/// base, up to the floor: no output texel is under `ops::vector::MIN_NORMAL_Z`, a
/// base texel already under it included, whatever the detail or `strength`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalBlend {
    #[serde(default = "one")]
    pub strength: Param,
}

impl Row for NormalBlend {
    const INFO: OpInfo = op(
        "normal_blend",
        "vector",
        arity(2, 2),
        &[socket(opt("strength", "number", Lit::Number(1.0)))],
    );
}

impl Op for NormalBlend {
    fn run(&self, cx: &Cx) -> Image {
        vector::normal_blend(cx.input(0), cx.input(1), cx.field(&self.strength))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("strength", &self.strength)]
    }
}

fn one() -> Param {
    Param::Num(1.0)
}
