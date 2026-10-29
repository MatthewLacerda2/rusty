//! src/scene/authoring/sanitize.rs — the float rules authoring setters share (#939):
//! a value from Lua, the editor or a scene file is finite and in range before it is
//! stored. A NaN or infinite input becomes the rule's fallback, never the field.
//!
//! Every setter uses these, so `+inf` from Lua (`math.huge`) is refused the same way
//! everywhere: it takes the fallback, as NaN does, and never reaches a field (#993).
//! The `sanitize` lint rule holds it (#1361): a `pub fn set_*` in this module that
//! takes an `f32` or a glam vector passes it to one of these (or to a sibling
//! setter that does), or says why not with `// lint: allow(sanitize) <why>`.

use glam::{Vec2, Vec3, Vec4};

use crate::core::curve::{ColorRange, Curve, Gradient, Range};

/// The smallest size a box side, radius, height or scale may have: one floor for the
/// one idea, shared by the nav obstacle and modifier, the LOD group, the canvas and
/// the UI gradient (#993). The character controller keeps its own, smaller one: its
/// floor is the collide-and-slide gap, not a visible extent.
pub const MIN_EXTENT: f32 = 1e-3;

/// `v`, or `0.0` when it is NaN or infinite.
pub(super) fn finite(v: f32) -> f32 {
    finite_or(v, 0.0)
}

/// `v`, or `fallback` when it is NaN or infinite. A setter whose field has no
/// natural zero passes the field's current value, so a bad input changes nothing.
pub(super) fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

/// `v` floored at `min`; `min` when `v` is NaN or infinite.
pub(super) fn at_least(v: f32, min: f32) -> f32 {
    if v.is_finite() {
        v.max(min)
    } else {
        min
    }
}

/// `v` floored at `0.0`; `0.0` when `v` is NaN or infinite.
pub(super) fn non_negative(v: f32) -> f32 {
    at_least(v, 0.0)
}

/// `v` clamped to `[lo, hi]`; `0.0` (itself clamped into the range) when `v` is
/// NaN or infinite.
pub(super) fn clamped(v: f32, lo: f32, hi: f32) -> f32 {
    finite(v).clamp(lo, hi)
}

/// `v` clamped to `[lo, hi]`, an infinity saturating to the end it points at;
/// `lo` when `v` is NaN (`f32::clamp` would keep it). For a bounded field whose
/// `+inf` reads as "as far as it goes", like a camera's field of view.
pub(super) fn saturated(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_nan() {
        lo
    } else {
        v.clamp(lo, hi)
    }
}

/// `v` as a whole count, rounded and clamped to `[lo, hi]`: a numeric knob (a tween,
/// a slider) writing a sample or step count. NaN or infinite takes `lo`.
pub(super) fn count(v: f32, lo: u32, hi: u32) -> u32 {
    if v.is_finite() {
        (v.round().clamp(lo as f32, hi as f32)) as u32
    } else {
        lo
    }
}

/// `v` clamped to `[0, 1]`; `0.0` when `v` is NaN or infinite.
pub(super) fn unit(v: f32) -> f32 {
    clamped(v, 0.0, 1.0)
}

/// An RGBA colour with each channel clamped to `[0, 1]`; transparent black when
/// any channel is NaN or infinite (a colour is refused whole, never mixed).
pub(super) fn unit_color(c: Vec4) -> Vec4 {
    finite4(c).clamp(Vec4::ZERO, Vec4::ONE)
}

/// `v`, or `Vec2::ZERO` when any component is NaN or infinite.
pub(super) fn finite2(v: Vec2) -> Vec2 {
    finite2_or(v, Vec2::ZERO)
}

/// `v`, or `fallback` when any component is NaN or infinite. A vector is refused
/// whole, as Unity refuses the assignment: never a point nobody authored.
pub(super) fn finite2_or(v: Vec2, fallback: Vec2) -> Vec2 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

/// `v`, or `Vec3::ZERO` when any component is NaN or infinite.
pub(super) fn finite3(v: Vec3) -> Vec3 {
    finite3_or(v, Vec3::ZERO)
}

/// `v`, or `fallback` when any component is NaN or infinite (refused whole).
pub(super) fn finite3_or(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

/// `v`, or `Vec4::ZERO` when any component is NaN or infinite.
pub(super) fn finite4(v: Vec4) -> Vec4 {
    if v.is_finite() {
        v
    } else {
        Vec4::ZERO
    }
}

/// `v`, or `fallback` when any element is NaN or infinite (refused whole).
pub(super) fn finite_array_or<const N: usize>(v: [f32; N], fallback: [f32; N]) -> [f32; N] {
    if v.iter().all(|x| x.is_finite()) {
        v
    } else {
        fallback
    }
}

/// [`finite_or`] for an `f64` (a Lua number not yet narrowed).
pub(super) fn finite_f64_or(v: f64, fallback: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

/// An optional size: `None` when it is absent, negative, NaN or infinite.
pub(super) fn optional_non_negative(v: Option<f32>) -> Option<f32> {
    v.filter(|v| v.is_finite() && *v >= 0.0)
}

/// `v`, or an error naming the first NaN or infinite value: a runtime param list is
/// refused whole, and the caller's `Result` says why.
pub(super) fn finite_values(v: Vec<f32>) -> Result<Vec<f32>, String> {
    match v.iter().find(|x| !x.is_finite()) {
        Some(bad) => Err(format!("a param value must be a finite number, got {bad}")),
        None => Ok(v),
    }
}

/// `r`, or `fallback` when either end is NaN or infinite (refused whole).
pub(super) fn range_or(r: Range, fallback: Range) -> Range {
    if r.min.is_finite() && r.max.is_finite() {
        r
    } else {
        fallback
    }
}

/// `c`, or `fallback` when any channel of either end is NaN or infinite.
pub(super) fn color_range_or(c: ColorRange, fallback: ColorRange) -> ColorRange {
    let finite = |v: &[f32; 4]| v.iter().all(|x| x.is_finite());
    if finite(&c.min) && finite(&c.max) {
        c
    } else {
        fallback
    }
}

/// `c` without the keys whose `t` or value is NaN or infinite, sorted by `t`.
pub(super) fn finite_curve(mut c: Curve) -> Curve {
    c.keys.retain(|k| k.t.is_finite() && k.value.is_finite());
    c.sort();
    c
}

/// `g` without the colour or alpha keys that carry a NaN or infinity, sorted.
pub(super) fn finite_gradient(mut g: Gradient) -> Gradient {
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
        g.color_keys.push(crate::core::curve::ColorKey {
            t: 0.5,
            color: [f32::NAN; 3],
        });
        g.alpha = Curve::from_keys(&[(0.0, f32::NAN)]);
        assert_eq!(finite_gradient(g), Gradient::default());
    }
}
