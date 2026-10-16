//! src/scene/authoring/sanitize.rs — the float rules authoring setters share (#939):
//! a value from Lua, the editor or a scene file is finite and in range before it is
//! stored. A NaN or infinite input becomes the rule's fallback, never the field.
//!
//! Every setter uses these, so `+inf` from Lua (`math.huge`) is refused the same way
//! everywhere: it takes the fallback, as NaN does, and never reaches a field (#993).

use glam::Vec4;

/// The smallest size a box side, radius, height or scale may have: one floor for the
/// one idea, shared by the nav obstacle and modifier, the LOD group, the canvas and
/// the UI gradient (#993). The character controller keeps its own, smaller one: its
/// floor is the collide-and-slide gap, not a visible extent.
pub const MIN_EXTENT: f32 = 1e-3;

/// `v`, or `0.0` when it is NaN or infinite.
pub(super) fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
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

/// `v` clamped to `[lo, hi]`; `0.0` when `v` is NaN or infinite.
pub(super) fn clamped(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        0.0
    }
}

/// `v` clamped to `[0, 1]`; `0.0` when `v` is NaN or infinite.
pub(super) fn unit(v: f32) -> f32 {
    clamped(v, 0.0, 1.0)
}

/// An RGBA colour with each channel clamped to `[0, 1]`.
pub(super) fn unit_color(c: Vec4) -> Vec4 {
    c.clamp(Vec4::ZERO, Vec4::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_inputs_take_the_fallback() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(finite(bad), 0.0);
            assert_eq!(at_least(bad, 2.0), 2.0);
            assert_eq!(non_negative(bad), 0.0);
            assert_eq!(unit(bad), 0.0);
            assert_eq!(clamped(bad, -5.0, 5.0), 0.0);
        }
    }

    #[test]
    fn finite_inputs_are_clamped_into_range() {
        assert_eq!(at_least(0.5, 2.0), 2.0);
        assert_eq!(non_negative(-1.0), 0.0);
        assert_eq!(unit(1.5), 1.0);
        assert_eq!(clamped(-9.0, -5.0, 5.0), -5.0);
        assert_eq!(
            unit_color(Vec4::new(-1.0, 0.5, 2.0, 1.0)),
            Vec4::new(0.0, 0.5, 1.0, 1.0)
        );
    }
}
