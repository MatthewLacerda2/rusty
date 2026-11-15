//! `color_ramp`'s `cardinal` and `b_spline` fill, ported from Blender's
//! `BKE_colorband_evaluate` (`colorband.cc`) and the `key_curve_position_weights` it
//! calls (`key.cc`): the four stops around a pixel weigh its colour, the end stops
//! standing in for the neighbours past either end.

use super::super::super::image_buf::Rgba;
use super::super::super::recipe::{RampInterpolation, RampStop};

/// Blender's cardinal tension (`fc` in `key_curve_position_weights`): the tangent at a
/// stop is `0.71` times the difference of its two neighbours.
const TENSION: f32 = 0.71;

/// The ramp at `t` (not NaN) by Blender's spline fill, where `above` is the index of
/// the first stop past `t` (`sorted.len()` when none is). Between stops `left` and
/// `right` the fill weighs those two and their outer neighbours. Before the first
/// stop it runs from a copy of it at position 0, after the last to a copy at 1, so,
/// as Blender's, a spline ramp is not held flat past its ends. The result is clamped
/// to `[0, 1]`, as Blender's (`clamp_v4`), which is where a `cardinal` overshoot
/// stops.
pub(super) fn sample(sorted: &[RampStop], above: usize, t: f32, kind: RampInterpolation) -> Rgba {
    let n = sorted.len();
    if n == 1 {
        return sorted[0].color;
    }
    let (left, right) = if above == n {
        let last = sorted[n - 1];
        (last, RampStop { pos: 1.0, ..last })
    } else if above == 0 {
        let first = sorted[0];
        (RampStop { pos: 0.0, ..first }, first)
    } else {
        (sorted[above - 1], sorted[above])
    };
    let outer_right = if above + 1 >= n {
        right
    } else {
        sorted[above + 1]
    };
    let outer_left = if above < 2 { left } else { sorted[above - 2] };
    // Blender runs the curve right to left: `fac` is 0 at `right`, 1 at `left`.
    let fac = if left.pos != right.pos {
        (t - right.pos) / (left.pos - right.pos)
    } else if above != n {
        0.0
    } else {
        1.0
    };
    let w = weights(fac.clamp(0.0, 1.0), kind);
    std::array::from_fn(|c| {
        let v = w[3] * outer_left.color[c]
            + w[2] * left.color[c]
            + w[1] * right.color[c]
            + w[0] * outer_right.color[c];
        v.clamp(0.0, 1.0)
    })
}

/// `key_curve_position_weights`: the weights of the four points `p0..p3` at `t` on
/// the segment from `p1` (`t = 0`) to `p2` (`t = 1`). The cardinal curve passes
/// through `p1` and `p2`; the uniform cubic B-spline only near them, its weights
/// never negative, so it never leaves the points' range.
pub(super) fn weights(t: f32, kind: RampInterpolation) -> [f32; 4] {
    let (t2, t3) = (t * t, t * t * t);
    if kind == RampInterpolation::Cardinal {
        let fc = TENSION;
        [
            -fc * t3 + 2.0 * fc * t2 - fc * t,
            (2.0 - fc) * t3 + (fc - 3.0) * t2 + 1.0,
            (fc - 2.0) * t3 + (3.0 - 2.0 * fc) * t2 + fc * t,
            fc * t3 - fc * t2,
        ]
    } else {
        [
            -0.166_666_66 * t3 + 0.5 * t2 - 0.5 * t + 0.166_666_66,
            0.5 * t3 - t2 + 0.666_666_7, // Blender's `0.66666666f`, the same f32
            -0.5 * t3 + 0.5 * t2 + 0.5 * t + 0.166_666_66,
            0.166_666_66 * t3,
        ]
    }
}
