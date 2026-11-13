//! The test-only `dummy` op (#1751): the whole cost of an op, and nothing else.
//!
//! It is the worked example `crates/procgen/src/CLAUDE.md`'s *Adding an op* points at:
//! this file, its row in the catalog and its `mod` line here are all an op adds
//! (plus, here, its math's file in `ops/` and that file's `mod` line). It has the shape a blend-like op takes: two inputs, a trailing optional
//! mask, and an enum `output` mode with a default. Its enum is declared here, and
//! its math sits in `ops/dummy.rs`, which names that enum by its catalog path
//! (`catalog::color::dummy::DummyOutput`): the shape an op takes when its math is
//! shared with siblings in `ops/<family>`. Every catalog-wide test
//! (`recipe/catalog_tests.rs`, `ops/arity_tests.rs`, `ops/link_tests.rs`, the Lua
//! enum-case test) runs on it, so it proves the contract a new op must meet.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::dummy;
use crate::recipe::{arity, choice, op, Lit, OpInfo, OpParam};

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
        dummy::keep(cx.input(0), cx.input(1), cx.optional(2), self.output)
    }
}
