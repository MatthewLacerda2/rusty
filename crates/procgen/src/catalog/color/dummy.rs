//! The test-only `dummy` op, the worked example for `crates/procgen/src/CLAUDE.md`'s *Adding
//! an op*: two inputs, a trailing optional mask and an enum `output` with a default. Every
//! catalog-wide test runs on it.
use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::dummy;
use crate::recipe::{arity, choice, op, Lit, OpInfo, OpParam};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DummyOutput {
    #[default]
    A,
    B,
}

/// Keep input `a` or `b` (per `output`) where the optional mask's red is above 0.5
/// (everywhere, when none is wired), the other input elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dummy {
    #[serde(default)]
    pub output: DummyOutput,
}

impl Row for Dummy {
    const INFO: OpInfo = op(
        "dummy",
        "color",
        arity(2, 3),
        &[OpParam {
            default: Some(Lit::Text("a")),
            ..choice("output", &["a", "b"])
        }],
    );
}

impl Op for Dummy {
    fn run(&self, cx: &Cx) -> Image {
        dummy::keep(cx.input(0), cx.input(1), cx.optional(2), self.output)
    }
}
