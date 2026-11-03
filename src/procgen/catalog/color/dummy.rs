//! The test-only `dummy` op (#1751): the whole cost of an op, and nothing else.
//!
//! It is the worked example `src/procgen/CLAUDE.md`'s *Adding an op* points at:
//! this file and its row in the catalog (plus its `mod` line here) are all an op
//! adds. It has the shape a blend-like op takes: two inputs, a trailing optional
//! mask, and an enum `output` mode with a default. Its math is small, so it lives
//! here; math siblings share goes in `ops/<family>`. Every catalog-wide test
//! (`recipe/catalog_tests.rs`, `ops/arity_tests.rs`, `ops/link_tests.rs`, the Lua
//! enum-case test) runs on it, so it proves the contract a new op must meet.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::recipe::{arity, choice, op, Lit, OpInfo, OpParam};

/// Which input the `dummy` op keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DummyOutput {
    /// The first input, where the mask says so.
    #[default]
    A,
    /// The second input, where the mask says so.
    B,
}

/// Keep input `a` or `b` (per `output`) where the optional third input's red is
/// above 0.5 (everywhere, when no mask is wired), and the other input elsewhere.
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
        let (a, b) = (cx.input(0), cx.input(1));
        let (keep, other) = match self.output {
            DummyOutput::A => (a, b),
            DummyOutput::B => (b, a),
        };
        let mut out = keep.clone();
        if let Some(mask) = cx.optional(2) {
            out.map_indexed(|i, p| match mask.pixels()[i][0] > 0.5 {
                true => p,
                false => other.pixels()[i],
            });
        }
        out
    }
}
