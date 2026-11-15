//! `color_ramp`'s math: a gradient of stops, looked up by binary search, and filled
//! between them as Blender's ColorRamp (`spline` holds its `cardinal` and `b_spline`).

mod spline;
#[cfg(test)]
mod spline_tests;
#[cfg(test)]
mod tests;

use super::super::image_buf::{Image, Rgba};
use super::super::recipe::{RampInterpolation, RampStop};

/// Map the input's red channel through a sorted gradient of [`RampStop`]s, filled
/// between stops by `interpolation`. With no stops the input passes through
/// unchanged.
pub fn color_ramp(mut img: Image, stops: &[RampStop], interpolation: RampInterpolation) -> Image {
    if stops.is_empty() {
        return img;
    }
    let mut sorted = stops.to_vec();
    sorted.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    // Checked once per ramp, not per pixel: a NaN position (which a recipe cannot
    // write) keeps the walk for every pixel, anything else the search.
    if sorted.iter().any(|s| pos(s).is_nan()) {
        img.map_in_place(|p| scan_ramp(&sorted, interpolation, p[0]));
    } else {
        img.map_in_place(|p| sample_ramp(&sorted, interpolation, p[0]));
    }
    img
}

/// Sample a sorted ramp at scalar `t`, clamping to the ends. `constant` takes the
/// last stop at or below `t` (Blender's: a stop's colour starts at its position);
/// `linear` and `ease` blend the bracketing stops, `ease` through a smoothstep;
/// `cardinal` and `b_spline` weigh the four stops around `t` ([`spline::sample`]). The
/// stops are found by binary search (#2002), so a lookup ramp of thousands of stops
/// costs a dozen comparisons a pixel. `sorted` holds no NaN position: [`color_ramp`]
/// sends such a ramp to [`scan_ramp`]'s linear walk, the two agreeing everywhere else.
fn sample_ramp(sorted: &[RampStop], interpolation: RampInterpolation, t: f32) -> Rgba {
    let ease = match interpolation {
        RampInterpolation::Linear => false,
        RampInterpolation::Ease => true,
        RampInterpolation::Constant => {
            let at_or_below = sorted.partition_point(|s| pos(s) <= t);
            return sorted[at_or_below.saturating_sub(1)].color;
        }
        RampInterpolation::Cardinal | RampInterpolation::BSpline => {
            if t.is_nan() {
                return sorted[sorted.len() - 1].color;
            }
            let above = sorted.partition_point(|s| pos(s) <= t);
            return spline::sample(sorted, above, t, interpolation);
        }
    };
    let last = sorted[sorted.len() - 1];
    if t <= sorted[0].pos {
        return sorted[0].color;
    }
    if t >= last.pos || t.is_nan() {
        return last.color;
    }
    let k = sorted.partition_point(|s| pos(s) < t) - 1;
    blend(sorted[k], sorted[k + 1], t, ease)
}

/// `t` between stops `a` and `b`, through a smoothstep when `ease`.
fn blend(a: RampStop, b: RampStop, t: f32, ease: bool) -> Rgba {
    let span = (b.pos - a.pos).max(1e-6);
    let f = (t - a.pos) / span;
    let f = if ease { f * f * (3.0 - 2.0 * f) } else { f };
    std::array::from_fn(|c| lerp(a.color[c], b.color[c], f))
}

/// [`sample_ramp`] by walking the stops in order: its reference, and its path for a
/// NaN position.
fn scan_ramp(sorted: &[RampStop], interpolation: RampInterpolation, t: f32) -> Rgba {
    let ease = match interpolation {
        RampInterpolation::Linear => false,
        RampInterpolation::Ease => true,
        RampInterpolation::Constant => {
            let held = sorted.iter().rev().find(|s| pos(s) <= t);
            return held.unwrap_or(&sorted[0]).color;
        }
        RampInterpolation::Cardinal | RampInterpolation::BSpline => {
            if t.is_nan() {
                return sorted[sorted.len() - 1].color;
            }
            // Blender's own walk: the first stop past `t`.
            let above = sorted.iter().position(|s| pos(s) > t);
            let above = above.unwrap_or(sorted.len());
            return spline::sample(sorted, above, t, interpolation);
        }
    };
    if t <= sorted[0].pos {
        return sorted[0].color;
    }
    let last = sorted[sorted.len() - 1];
    if t >= last.pos {
        return last.color;
    }
    let window = sorted
        .windows(2)
        .find(|w| t >= pos(&w[0]) && t <= pos(&w[1]));
    window.map_or(last.color, |w| blend(w[0], w[1], t, ease))
}

/// A stop's position, read through one place so a test can count how many stops a
/// lookup reads (the work a ramp costs per pixel).
#[inline]
fn pos(stop: &RampStop) -> f32 {
    #[cfg(test)]
    PROBES.with(|n| n.set(n.get() + 1));
    stop.pos
}

#[cfg(test)]
thread_local! {
    static PROBES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
