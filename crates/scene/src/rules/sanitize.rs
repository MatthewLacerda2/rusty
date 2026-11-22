use glam::{Vec2, Vec3, Vec4};

use rusty_core::curve::{ColorRange, Curve, Gradient, Range};

/// The floor of a size (a box side, a radius, a scale), in its own field's unit.
pub const MIN_EXTENT: f32 = 1e-3;

/// `v`, or `0.0` when it is NaN or infinite.
pub fn finite(v: f32) -> f32 {
    finite_or(v, 0.0)
}

pub fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

pub fn at_least(v: f32, min: f32) -> f32 {
    if v.is_finite() {
        v.max(min)
    } else {
        min
    }
}

/// `v` floored at `0.0`; `0.0` when `v` is NaN or infinite.
pub fn non_negative(v: f32) -> f32 {
    at_least(v, 0.0)
}

/// `v` clamped to `[lo, hi]`; `0.0` (itself clamped into the range) when `v` is
/// NaN or infinite.
pub fn clamped(v: f32, lo: f32, hi: f32) -> f32 {
    finite(v).clamp(lo, hi)
}

// lint: allow(sanitize) holds `v`: the clamp turns ±inf into a bound and the `is_nan` test catches NaN
pub fn saturated(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_nan() {
        lo
    } else {
        v.clamp(lo, hi)
    }
}

pub fn unit(v: f32) -> f32 {
    clamped(v, 0.0, 1.0)
}

pub fn unit_color(c: Vec4) -> Vec4 {
    finite4(c).clamp(Vec4::ZERO, Vec4::ONE)
}

/// `v`, or `Vec2::ZERO` when any component is NaN or infinite.
pub fn finite2(v: Vec2) -> Vec2 {
    finite2_or(v, Vec2::ZERO)
}

pub fn finite2_or(v: Vec2, fallback: Vec2) -> Vec2 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

/// `v`, or `Vec3::ZERO` when any component is NaN or infinite.
pub fn finite3(v: Vec3) -> Vec3 {
    finite3_or(v, Vec3::ZERO)
}

pub fn finite3_or(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

// lint: allow(sanitize) holds `v`: the match tests its colour (`c.is_finite()`)
pub fn optional_color_or(v: Option<Vec3>, current: Option<Vec3>) -> Option<Vec3> {
    match v {
        Some(c) if c.is_finite() => Some(c.max(Vec3::ZERO)),
        Some(_) => current,
        None => None,
    }
}

/// `v`, or `Vec4::ZERO` when any component is NaN or infinite.
pub fn finite4(v: Vec4) -> Vec4 {
    if v.is_finite() {
        v
    } else {
        Vec4::ZERO
    }
}

pub fn finite_array_or<const N: usize>(v: [f32; N], fallback: [f32; N]) -> [f32; N] {
    if v.iter().all(|x| x.is_finite()) {
        v
    } else {
        fallback
    }
}

/// [`finite_or`] for an `f64` (a Lua number not yet narrowed).
pub fn finite_f64_or(v: f64, fallback: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

pub fn optional_non_negative(v: Option<f32>) -> Option<f32> {
    v.filter(|v| v.is_finite() && *v >= 0.0)
}

pub fn finite_values(v: Vec<f32>) -> Result<Vec<f32>, String> {
    match v.iter().find(|x| !x.is_finite()) {
        Some(bad) => Err(format!("a param value must be a finite number, got {bad}")),
        None => Ok(v),
    }
}

pub fn range_or(r: Range, fallback: Range) -> Range {
    if r.min.is_finite() && r.max.is_finite() {
        r
    } else {
        fallback
    }
}

pub fn color_range_or(c: ColorRange, fallback: ColorRange) -> ColorRange {
    if c.min.iter().chain(&c.max).all(|x| x.is_finite()) {
        c
    } else {
        fallback
    }
}

pub fn finite_curve(mut c: Curve) -> Curve {
    c.keys.retain(|k| k.t.is_finite() && k.value.is_finite());
    c.sort();
    c
}

pub fn finite_gradient(mut g: Gradient) -> Gradient {
    g.color_keys
        .retain(|k| k.t.is_finite() && k.color.iter().all(|x| x.is_finite()));
    g.alpha = finite_curve(g.alpha);
    g.sort();
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_inputs_take_the_fallback() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(finite(bad), 0.0);
            assert_eq!(finite_or(bad, 1.5), 1.5);
            assert_eq!(at_least(bad, 2.0), 2.0);
            assert_eq!(non_negative(bad), 0.0);
            assert_eq!(unit(bad), 0.0);
            assert_eq!(clamped(bad, -5.0, 5.0), 0.0);
            assert!((1.0..=9.0).contains(&saturated(bad, 1.0, 9.0)));
            assert_eq!(
                clamped(bad, 0.01, 10.0),
                0.01,
                "the fallback stays in range"
            );
        }
    }

    #[test]
    fn finite_inputs_are_clamped_into_range() {
        assert_eq!(finite_or(-3.0, 1.5), -3.0);
        assert_eq!(at_least(0.5, 2.0), 2.0);
        assert_eq!(non_negative(-1.0), 0.0);
        assert_eq!(unit(1.5), 1.0);
        assert_eq!(clamped(-9.0, -5.0, 5.0), -5.0);
        assert_eq!(saturated(f32::NAN, 1.0, 9.0), 1.0);
        assert_eq!(saturated(f32::INFINITY, 1.0, 9.0), 9.0);
        assert_eq!(saturated(4.0, 1.0, 9.0), 4.0);
        assert_eq!(
            unit_color(Vec4::new(-1.0, 0.5, 2.0, 1.0)),
            Vec4::new(0.0, 0.5, 1.0, 1.0)
        );
    }

    #[test]
    fn vectors_and_lists_are_refused_whole() {
        let bad = Vec4::new(f32::NAN, 0.5, 2.0, 1.0);
        assert_eq!(unit_color(bad), Vec4::ZERO);
        assert_eq!(
            unit_color(Vec4::new(-1.0, 0.5, 2.0, 1.0)),
            Vec4::new(0.0, 0.5, 1.0, 1.0)
        );
        assert_eq!(finite4(bad), Vec4::ZERO);
        let kept = Vec3::splat(7.0);
        assert_eq!(finite3_or(Vec3::new(5.0, f32::NAN, 9.0), kept), kept);
        assert_eq!(finite3(Vec3::new(1.0, f32::INFINITY, 0.0)), Vec3::ZERO);
        assert_eq!(finite2_or(Vec2::new(1.0, f32::NAN), Vec2::ONE), Vec2::ONE);
        assert_eq!(finite2(Vec2::new(3.0, 4.0)), Vec2::new(3.0, 4.0));
        assert_eq!(finite_array_or([1.0, f32::NAN], [2.0, 2.0]), [2.0, 2.0]);
        assert_eq!(finite_f64_or(f64::NAN, 50.0), 50.0);
        assert_eq!(optional_non_negative(Some(f32::INFINITY)), None);
        assert_eq!(optional_non_negative(Some(3.0)), Some(3.0));
        assert!(finite_values(vec![1.0, f32::NAN]).is_err());
        assert_eq!(finite_values(vec![1.0]), Ok(vec![1.0]));
    }

    #[test]
    fn ranges_and_curves_drop_what_is_not_finite() {
        let keep = Range::new(1.0, 2.0);
        assert_eq!(range_or(Range::new(0.0, f32::INFINITY), keep), keep);
        let c = ColorRange::constant([1.0; 4]);
        let bad = ColorRange::constant([f32::NAN, 1.0, 1.0, 1.0]);
        assert_eq!(color_range_or(bad, c), c);
        let curve = finite_curve(Curve::from_keys(&[
            (1.0, 2.0),
            (f32::NAN, 1.0),
            (0.0, f32::INFINITY),
        ]));
        assert_eq!(curve, Curve::from_keys(&[(1.0, 2.0)]));
        let mut g = Gradient::default();
        g.color_keys.push(rusty_core::curve::ColorKey {
            t: 0.5,
            color: [f32::NAN; 3],
        });
        g.alpha = Curve::from_keys(&[(0.0, f32::NAN)]);
        assert_eq!(finite_gradient(g), Gradient::default());
    }
}
