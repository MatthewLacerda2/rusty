//! The `math` op: per-channel arithmetic against a constant or a second image.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::math;
use crate::procgen::recipe::{arity, choice, op, opt, Arity, Lit, MathOp, OpInfo, ONE};

const FUNCS: &[&str] = &[
    "add", "subtract", "multiply", "divide", "power", "min", "max", "abs", "fract", "sqrt",
];

/// Per-channel math. The right-hand side is the constant `value`, or, when a second
/// input is wired, that input's matching channel per pixel (`value` is then ignored,
/// Blender's Math with both sockets linked). `value` defaults to 0.5, Blender's
/// socket default, so a wired second input needs no dummy `value`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Math {
    pub func: MathOp,
    #[serde(default = "half")]
    pub value: f32,
}

impl Row for Math {
    const INFO: OpInfo = op(
        "math",
        "math",
        arity(1, 2),
        &[
            choice("func", FUNCS),
            opt("value", "number", Lit::Number(0.5)),
        ],
    );
}

impl Op for Math {
    fn run(&self, cx: &Cx) -> Image {
        math::math(cx.own(), cx.optional(1), self.func, self.value)
    }

    /// A unary func (`abs`, `fract`, `sqrt`) has no right-hand side to read a
    /// second input into.
    fn arity(&self) -> Option<Arity> {
        self.func.is_unary().then_some(ONE)
    }

    fn func(&self) -> Option<String> {
        let func = serde_json::to_value(self.func).ok()?;
        func.as_str().map(str::to_string)
    }
}

fn half() -> f32 {
    0.5
}
