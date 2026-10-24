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

/// `v` clamped to `[0, 1]`; `0.0` when `v` is NaN or infinite.
pub(super) fn unit(v: f32) -> f32 {
    clamped(v, 0.0, 1.0)
}

/// An RGBA colour with each channel clamped to `[0, 1]`; a NaN or infinite
/// channel is `0.0`.
pub(super) fn unit_color(c: Vec4) -> Vec4 {
    Vec4::new(unit(c.x), unit(c.y), unit(c.z), unit(c.w))
}

/// `v` with each NaN or infinite component `0.0`.
pub(super) fn finite2(v: Vec2) -> Vec2 {
    finite2_or(v, Vec2::ZERO)
}

/// `v` with each NaN or infinite component taken from `fallback`.
pub(super) fn finite2_or(v: Vec2, fallback: Vec2) -> Vec2 {
    Vec2::new(finite_or(v.x, fallback.x), finite_or(v.y, fallback.y))
}

/// `v` with each NaN or infinite component `0.0`.
pub(super) fn finite3(v: Vec3) -> Vec3 {
    finite3_or(v, Vec3::ZERO)
}

/// `v` with each NaN or infinite component taken from `fallback`.
pub(super) fn finite3_or(v: Vec3, fallback: Vec3) -> Vec3 {
    Vec3::new(
        finite_or(v.x, fallback.x),
        finite_or(v.y, fallback.y),
        finite_or(v.z, fallback.z),
    )
}

/// `v` with each NaN or infinite component `0.0`.
pub(super) fn finite4(v: Vec4) -> Vec4 {
    Vec4::new(finite(v.x), finite(v.y), finite(v.z), finite(v.w))
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
        assert_eq!(
            unit_color(Vec4::new(-1.0, 0.5, 2.0, 1.0)),
            Vec4::new(0.0, 0.5, 1.0, 1.0)
        );
    }

    #[test]
    fn vectors_sanitize_each_component() {
        let bad = Vec4::new(f32::NAN, 0.5, f32::INFINITY, f32::NEG_INFINITY);
        assert_eq!(unit_color(bad), Vec4::new(0.0, 0.5, 0.0, 0.0));
        assert_eq!(finite4(bad), Vec4::new(0.0, 0.5, 0.0, 0.0));
        assert_eq!(finite3(bad.truncate()), Vec3::new(0.0, 0.5, 0.0));
        let kept = finite3_or(bad.truncate(), Vec3::splat(7.0));
        assert_eq!(kept, Vec3::new(7.0, 0.5, 7.0));
        assert_eq!(finite2(bad.truncate().truncate()), Vec2::new(0.0, 0.5));
        assert_eq!(finite2_or(Vec2::NAN, Vec2::ONE), Vec2::ONE);
    }
}
