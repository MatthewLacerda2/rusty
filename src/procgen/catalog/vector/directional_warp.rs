//! The `directional_warp` op: push an image along one angle by a grayscale map
//! (Substance Designer's *Directional Warp*, #1739).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::recipe::{arity, op, opt, req, Lit, OpInfo};

/// Input 1 moved `strength × red` tiles along `angle` (turns from the horizontal,
/// 0 pushing right, 0.25 down), the red read from input 2. Black moves nothing and
/// white the full `strength`, as in Substance's node; a negative strength pushes
/// the other way. It is the grayscale, one-direction counterpart of `warp`, which
/// reads two channels around mid-grey. Both reads are bilinear and wrapped, so two
/// tiling inputs give a tiling map at any angle. A NaN strength moves nothing; it
/// caps at ±16 tiles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectionalWarp {
    pub strength: f32,
    #[serde(default)]
    pub angle: f32,
}

impl Row for DirectionalWarp {
    const INFO: OpInfo = op(
        "directional_warp",
        "vector",
        arity(2, 2),
        &[
            req("strength", "number"),
            opt("angle", "number", Lit::Number(0.0)),
        ],
    );
}

impl Op for DirectionalWarp {
    fn run(&self, cx: &Cx) -> Image {
        directional_warp(cx.input(0), cx.input(1), self.strength, self.angle)
    }
}

/// The furthest push, in tiles: a few tiles is already a wrap many times over, and
/// a bound keeps the sample point's `f32` precise.
pub const MAX_STRENGTH: f32 = 16.0;

/// `img` pushed `strength × by.red` tiles along `angle` turns.
pub fn directional_warp(img: &Image, by: &Image, strength: f32, angle: f32) -> Image {
    let strength = if strength.is_nan() {
        0.0
    } else {
        strength.clamp(-MAX_STRENGTH, MAX_STRENGTH)
    };
    let turn = if angle.is_finite() { angle } else { 0.0 };
    let (sin, cos) = (turn * std::f32::consts::TAU).sin_cos();
    Image::fill_uv(img.size(), |u, v| {
        let d = strength * by.sample_bilinear_wrapped(u, v)[0];
        // The content moves along the angle, so each pixel reads from behind it.
        img.sample_bilinear_wrapped(u - cos * d, v - sin * d)
    })
}

#[cfg(test)]
#[path = "directional_warp_tests.rs"]
mod tests;
