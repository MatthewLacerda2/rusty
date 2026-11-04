//! The `levels` op: `map_range` with a midtone gamma (Substance Designer's
//! *Levels*, #1743).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::recipe::{op, opt, Lit, OpInfo, ONE};

/// Per RGB channel: `[in_low, in_high]` → `[0, 1]` (clamped), raised to `1 / gamma`,
/// then onto `[out_low, out_high]`. A `gamma` above 1 lifts the midtones, below 1
/// sinks them, and neither moves the ends. Every param defaults to a pass-through.
/// Alpha untouched.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Levels {
    #[serde(default)]
    pub in_low: f32,
    #[serde(default = "one")]
    pub in_high: f32,
    #[serde(default = "one")]
    pub gamma: f32,
    #[serde(default)]
    pub out_low: f32,
    #[serde(default = "one")]
    pub out_high: f32,
}

impl Row for Levels {
    const INFO: OpInfo = op(
        "levels",
        "color",
        ONE,
        &[
            opt("in_low", "number", Lit::Number(0.0)),
            opt("in_high", "number", Lit::Number(1.0)),
            opt("gamma", "number", Lit::Number(1.0)),
            opt("out_low", "number", Lit::Number(0.0)),
            opt("out_high", "number", Lit::Number(1.0)),
        ],
    );
}

impl Op for Levels {
    fn run(&self, cx: &Cx) -> Image {
        let mut img = cx.own();
        img.map_in_place(|p| {
            let f = |c| self.level(c);
            [f(p[0]), f(p[1]), f(p[2]), p[3]]
        });
        img
    }
}

impl Levels {
    /// One channel through the levels. An empty input range (`in_low == in_high`)
    /// is a step, white at and above it; a `gamma` at or below 0 reads as `1e-4`.
    fn level(&self, c: f32) -> f32 {
        let span = self.in_high - self.in_low;
        let t = if span.abs() < 1e-6 {
            f32::from(u8::from(c >= self.in_low))
        } else {
            ((c - self.in_low) / span).clamp(0.0, 1.0)
        };
        let t = t.powf(1.0 / self.gamma.max(1e-4));
        self.out_low + (self.out_high - self.out_low) * t
    }
}

fn one() -> f32 {
    1.0
}

#[cfg(test)]
#[path = "levels_tests.rs"]
mod tests;
