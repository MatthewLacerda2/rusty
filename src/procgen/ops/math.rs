//! src/procgen/ops/math.rs — converter / math ops (one input; `math` takes two).
//!
//! Scalar transforms applied per channel: arithmetic against a constant or a second
//! image, range remap, clamp, and the RGB→luminance collapse. All pixel→pixel, run
//! through [`Image::map_in_place`] (or [`Image::for_each_row`] with a second image).

use super::super::image_buf::{Image, Rgba};
use super::super::recipe::MathOp;

/// Apply a [`MathOp`] per RGB channel; alpha is left untouched. The right-hand side
/// is `rhs`'s matching channel per pixel when it is wired, else the constant `value`.
pub fn math(mut img: Image, rhs: Option<&Image>, op: MathOp, value: f32) -> Image {
    let Some(rhs) = rhs else {
        img.map_in_place(|p| {
            let f = |c| apply(op, c, value);
            [f(p[0]), f(p[1]), f(p[2]), p[3]]
        });
        return img;
    };
    let w = img.resolution() as usize;
    img.for_each_row(|y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            let q = rhs.pixels()[y * w + x];
            *p = [
                apply(op, p[0], q[0]),
                apply(op, p[1], q[1]),
                apply(op, p[2], q[2]),
                p[3],
            ];
        }
    });
    img
}

/// `c op v` for one channel. Division by zero gives 0, a negative base to `power` or
/// `sqrt` reads as 0.
fn apply(op: MathOp, c: f32, v: f32) -> f32 {
    match op {
        MathOp::Add => c + v,
        MathOp::Subtract => c - v,
        MathOp::Multiply => c * v,
        MathOp::Divide => {
            if v.abs() < 1e-9 {
                0.0
            } else {
                c / v
            }
        }
        MathOp::Power => c.max(0.0).powf(v),
        MathOp::Min => c.min(v),
        MathOp::Max => c.max(v),
        MathOp::Abs => c.abs(),
        MathOp::Fract => c.rem_euclid(1.0),
        MathOp::Sqrt => c.max(0.0).sqrt(),
    }
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
    crate::core::color::luminance([p[0], p[1], p[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every binary func on known operands, each value telling the func apart from
    /// a swapped operator (`3 + 2` is not `3 − 2`, `3 × 2` or `3 / 2`).
    #[test]
    fn each_func_gives_its_result_on_known_operands() {
        let cases = [
            (MathOp::Add, 5.0),
            (MathOp::Subtract, 1.0),
            (MathOp::Multiply, 6.0),
            (MathOp::Divide, 1.5),
            (MathOp::Power, 9.0),
            (MathOp::Min, 2.0),
            (MathOp::Max, 3.0),
        ];
        for (op, want) in cases {
            assert_eq!(apply(op, 3.0, 2.0), want, "{op:?}");
        }
        assert_eq!(apply(MathOp::Abs, -3.0, 2.0), 3.0);
        assert_eq!(apply(MathOp::Fract, -0.25, 2.0), 0.75);
        assert_eq!(apply(MathOp::Sqrt, 9.0, 2.0), 3.0);
    }

    #[test]
    fn divide_is_zero_only_below_its_guard() {
        assert_eq!(apply(MathOp::Divide, 1.0, 0.0), 0.0);
        assert_eq!(apply(MathOp::Divide, 1.0, -1e-10), 0.0);
        // Exactly at the guard the division runs.
        assert_eq!(apply(MathOp::Divide, 1.0, 1e-9), 1.0 / 1e-9);
        assert_eq!(apply(MathOp::Divide, 1.0, -4.0), -0.25);
    }

    #[test]
    fn a_wired_rhs_is_read_per_channel() {
        let a = Image::filled(2, [3.0, 3.0, 3.0, 0.5]);
        let b = Image::filled(2, [2.0, 1.0, 4.0, 9.0]);
        let out = math(a, Some(&b), MathOp::Subtract, 0.5);
        assert_eq!(out.pixels()[3], [1.0, 2.0, -1.0, 0.5]);
    }
}
