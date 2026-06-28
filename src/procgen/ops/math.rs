//! src/procgen/ops/math.rs — converter / math ops (one input).
//!
//! Scalar transforms applied per channel: arithmetic against a constant, range
//! remap, clamp, and the RGB→luminance collapse. All pixel→pixel, run through
//! [`Image::map_in_place`].

use super::super::image_buf::{Image, Rgba};
use super::super::recipe::MathOp;

/// Apply a scalar [`MathOp`] against the constant `value`, per RGB channel; alpha is
/// left untouched.
pub fn math(mut img: Image, op: MathOp, value: f32) -> Image {
    let apply = |c: f32| match op {
        MathOp::Add => c + value,
        MathOp::Subtract => c - value,
        MathOp::Multiply => c * value,
        MathOp::Divide => {
            if value.abs() < 1e-9 {
                0.0
            } else {
                c / value
            }
        }
        MathOp::Power => c.max(0.0).powf(value),
        MathOp::Min => c.min(value),
        MathOp::Max => c.max(value),
        MathOp::Abs => c.abs(),
        MathOp::Fract => c.rem_euclid(1.0),
        MathOp::Sqrt => c.max(0.0).sqrt(),
    };
    img.map_in_place(|p| [apply(p[0]), apply(p[1]), apply(p[2]), p[3]]);
    img
}

/// Remap each RGB channel from `[from_min, from_max]` to `[to_min, to_max]`
/// (unclamped, like Blender's default Map Range). Alpha untouched.
pub fn map_range(mut img: Image, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> Image {
    let span = from_max - from_min;
    let remap = |c: f32| {
        if span.abs() < 1e-9 {
            to_min
        } else {
            to_min + (c - from_min) / span * (to_max - to_min)
        }
    };
    img.map_in_place(|p| [remap(p[0]), remap(p[1]), remap(p[2]), p[3]]);
    img
}

/// Clamp each RGB channel into `[min, max]`; alpha untouched.
pub fn clamp(mut img: Image, min: f32, max: f32) -> Image {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    img.map_in_place(|p| {
        [
            p[0].clamp(lo, hi),
            p[1].clamp(lo, hi),
            p[2].clamp(lo, hi),
            p[3],
        ]
    });
    img
}

/// Collapse RGB to Rec. 709 luminance, written to all three channels (so the result
/// reads as grayscale however it is sampled). Alpha untouched.
pub fn rgb_to_bw(mut img: Image) -> Image {
    img.map_in_place(|p| {
        let y = luminance(p);
        [y, y, y, p[3]]
    });
    img
}

/// Rec. 709 luma of a linear RGBA pixel.
#[inline]
pub fn luminance(p: Rgba) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}
