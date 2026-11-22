#[cfg(test)]
mod pinned_tests;
mod spline;
#[cfg(test)]
mod spline_tests;
#[cfg(test)]
mod tests;

use super::super::image_buf::{Image, Rgba};
use super::super::recipe::{RampInterpolation, RampStop};
use spline::Spline;

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

/// The end clamp of `linear`, `constant` and `ease` must stay below the spline branch:
/// `cardinal` and `b_spline` are not clamped to the ends, they run on toward copies of the end
/// stops ([`spline::fill`]). `sorted` holds no NaN position: [`color_ramp`]
/// sends such a ramp to [`scan_ramp`]'s linear walk.
fn sample_ramp(sorted: &[RampStop], interpolation: RampInterpolation, t: f32) -> Rgba {
    let above = |t| sorted.partition_point(|s| pos(s) <= t);
    let ease = match interpolation {
        RampInterpolation::Linear => false,
        RampInterpolation::Ease => true,
        RampInterpolation::Constant => return sorted[above(t).saturating_sub(1)].color,
        RampInterpolation::Cardinal => return spline::fill(sorted, Spline::Cardinal, t, above),
        RampInterpolation::BSpline => return spline::fill(sorted, Spline::BSpline, t, above),
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

fn blend(a: RampStop, b: RampStop, t: f32, ease: bool) -> Rgba {
    let span = (b.pos - a.pos).max(1e-6);
    let f = (t - a.pos) / span;
    let f = if ease { f * f * (3.0 - 2.0 * f) } else { f };
    std::array::from_fn(|c| lerp(a.color[c], b.color[c], f))
}

fn scan_ramp(sorted: &[RampStop], interpolation: RampInterpolation, t: f32) -> Rgba {
    // Blender's own walk: the first stop past `t`.
    let above = |t| {
        sorted
            .iter()
            .position(|s| pos(s) > t)
            .unwrap_or(sorted.len())
    };
    let ease = match interpolation {
        RampInterpolation::Linear => false,
        RampInterpolation::Ease => true,
        RampInterpolation::Constant => {
            let held = sorted.iter().rev().find(|s| pos(s) <= t);
            return held.unwrap_or(&sorted[0]).color;
        }
        RampInterpolation::Cardinal => return spline::fill(sorted, Spline::Cardinal, t, above),
        RampInterpolation::BSpline => return spline::fill(sorted, Spline::BSpline, t, above),
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

/// The searches read positions through here, so a test counts the stops a lookup reads.
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
