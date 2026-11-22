use super::super::image_buf::{Image, Rgba};
use super::super::recipe::MathOp;

pub fn math(mut img: Image, rhs: Option<&Image>, op: MathOp, value: f32) -> Image {
    let Some(rhs) = rhs else {
        img.map_in_place(|p| {
            let f = |c| apply(op, c, value);
            [f(p[0]), f(p[1]), f(p[2]), p[3]]
        });
        return img;
    };
    let w = img.width() as usize;
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

/// Division by zero gives 0; a non-positive or NaN base to `power` or `sqrt` reads +0.
/// Bits fixed across platforms: `f32::min`/`max` leave a zero's sign unspecified (x86, aarch64
/// differ), so the comparison is written out.
pub(crate) fn apply(op: MathOp, c: f32, v: f32) -> f32 {
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
        MathOp::Power => non_negative(c).powf(v),
        MathOp::Min => least(c, v),
        MathOp::Max => greatest(c, v),
        MathOp::Abs => c.abs(),
        MathOp::Fract => c.rem_euclid(1.0),
        MathOp::Sqrt => non_negative(c).sqrt(),
        MathOp::Sin => c.sin(),
        MathOp::Cos => c.cos(),
        MathOp::Atan2 => c.atan2(v),
    }
}

fn non_negative(c: f32) -> f32 {
    if c > 0.0 {
        c
    } else {
        0.0
    }
}

/// The lesser of `c` and `v`, as `f32::min` (a NaN loses to a number), with -0
/// below +0 whichever comes first.
fn least(c: f32, v: f32) -> f32 {
    if v.is_nan() || c < v || (c == v && c.is_sign_negative()) {
        c
    } else {
        v
    }
}

fn greatest(c: f32, v: f32) -> f32 {
    if v.is_nan() || c > v || (c == v && c.is_sign_positive()) {
        c
    } else {
        v
    }
}

/// Unclamped, as Blender's Map Range by default.
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

pub fn clamp(mut img: Image, min: f32, max: f32) -> Image {
    let f = |c| clamp_value(c, min, max);
    img.map_in_place(|p| [f(p[0]), f(p[1]), f(p[2]), p[3]]);
    img
}

/// `c` clamped into `[min, max]`, bounds swapped when `min > max`; a NaN bound is no bound, not a
/// panic.
pub(crate) fn clamp_value(c: f32, min: f32, max: f32) -> f32 {
    let min = if min.is_nan() { f32::NEG_INFINITY } else { min };
    let max = if max.is_nan() { f32::INFINITY } else { max };
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    if c < lo {
        lo
    } else if c > hi {
        hi
    } else {
        c
    }
}

pub fn rgb_to_bw(mut img: Image) -> Image {
    img.map_in_place(|p| {
        let y = luminance(p);
        [y, y, y, p[3]]
    });
    img
}

#[inline]
pub fn luminance(p: Rgba) -> f32 {
    rusty_core::color::luminance([p[0], p[1], p[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn trig_funcs_work_in_radians() {
        use std::f32::consts::{FRAC_PI_2, PI};
        assert!((apply(MathOp::Sin, FRAC_PI_2, 9.0) - 1.0).abs() < 1e-6);
        assert!((apply(MathOp::Cos, PI, 9.0) + 1.0).abs() < 1e-6);
        // Within an ulp: macOS's libm gives atan2(0, -1) one ulp under PI.
        let near = |got: f32, want: f32| (got - want).abs() < 1e-6;
        assert!(near(apply(MathOp::Atan2, 1.0, 0.0), FRAC_PI_2));
        assert!(near(apply(MathOp::Atan2, 0.0, -1.0), PI));
        assert!(near(apply(MathOp::Atan2, -1.0, 0.0), -FRAC_PI_2));
    }

    #[test]
    fn divide_is_zero_only_below_its_guard() {
        assert_eq!(apply(MathOp::Divide, 1.0, 0.0), 0.0);
        assert_eq!(apply(MathOp::Divide, 1.0, -1e-10), 0.0);
        assert_eq!(apply(MathOp::Divide, 1.0, 1e-9), 1.0 / 1e-9);
        assert_eq!(apply(MathOp::Divide, 1.0, -4.0), -0.25);
    }

    #[test]
    fn a_zeros_sign_is_fixed_on_every_platform() {
        let neg = (-0.0f32).to_bits();
        let out = clamp(Image::filled(1, [-0.0, -0.0, 0.0, 1.0]), 0.0, 1.0);
        let got = out.pixels()[0].map(f32::to_bits);
        assert_eq!(got[..3], [neg, neg, 0], "a -0 inside the bounds is kept");
        for (c, v) in [(-0.0, 0.0), (0.0, -0.0)] {
            assert_eq!(apply(MathOp::Min, c, v).to_bits(), neg, "-0 is the lesser");
            assert_eq!(apply(MathOp::Max, c, v).to_bits(), 0, "+0 is the greater");
        }
        assert_eq!(
            apply(MathOp::Min, f32::NAN, 2.0),
            2.0,
            "a NaN loses, as f32::min"
        );
        assert_eq!(apply(MathOp::Max, 2.0, f32::NAN), 2.0);
        assert_eq!(
            apply(MathOp::Power, -0.0, -1.0),
            f32::INFINITY,
            "base read as +0"
        );
        assert_eq!(apply(MathOp::Sqrt, -0.0, 0.0).to_bits(), 0);
    }

    #[test]
    fn a_wired_rhs_is_read_per_channel() {
        let a = Image::filled(2, [3.0, 3.0, 3.0, 0.5]);
        let b = Image::filled(2, [2.0, 1.0, 4.0, 9.0]);
        let out = math(a, Some(&b), MathOp::Subtract, 0.5);
        assert_eq!(out.pixels()[3], [1.0, 2.0, -1.0, 0.5]);
    }
}
