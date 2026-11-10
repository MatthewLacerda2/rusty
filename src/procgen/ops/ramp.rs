//! src/procgen/ops/ramp.rs — `color_ramp`'s math: a gradient of stops, looked up by
//! binary search (#2002).

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
    img.map_in_place(|p| sample_ramp(&sorted, interpolation, p[0]));
    img
}

/// Sample a sorted ramp at scalar `t`, clamping to the ends. `constant` takes the
/// last stop at or below `t` (Blender's: a stop's colour starts at its position);
/// `linear` and `ease` blend the bracketing stops, `ease` through a smoothstep. The
/// stops are found by binary search (#2002), so a lookup ramp of thousands of stops
/// costs a dozen comparisons a pixel; a ramp with a NaN position, which a recipe
/// cannot write, keeps [`scan_ramp`]'s linear walk, the two agreeing everywhere else.
fn sample_ramp(sorted: &[RampStop], interpolation: RampInterpolation, t: f32) -> Rgba {
    if sorted.iter().any(|s| s.pos.is_nan()) {
        return scan_ramp(sorted, interpolation, t);
    }
    // Stops at or below `t`, and strictly below it.
    let at_or_below = sorted.partition_point(|s| s.pos <= t);
    let ease = match interpolation {
        RampInterpolation::Linear => false,
        RampInterpolation::Ease => true,
        RampInterpolation::Constant => return sorted[at_or_below.saturating_sub(1)].color,
    };
    let last = sorted[sorted.len() - 1];
    if t <= sorted[0].pos {
        return sorted[0].color;
    }
    if t >= last.pos || t.is_nan() {
        return last.color;
    }
    let k = sorted.partition_point(|s| s.pos < t) - 1;
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
            let held = sorted.iter().rev().find(|s| s.pos <= t);
            return held.unwrap_or(&sorted[0]).color;
        }
    };
    if t <= sorted[0].pos {
        return sorted[0].color;
    }
    let last = sorted[sorted.len() - 1];
    if t >= last.pos {
        return last.color;
    }
    let window = sorted.windows(2).find(|w| t >= w[0].pos && t <= w[1].pos);
    window.map_or(last.color, |w| blend(w[0], w[1], t, ease))
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(pos: f32, v: f32) -> RampStop {
        RampStop {
            pos,
            color: [v, v, v, 1.0],
        }
    }

    /// A ramp from black at 0.2 through mid-grey at 0.5 to white at 0.8, sampled
    /// across `[0, 1]`.
    fn ramp(interpolation: RampInterpolation) -> Vec<f32> {
        let stops = [stop(0.8, 1.0), stop(0.2, 0.0), stop(0.5, 0.5)];
        let img = Image::fill_uv(64, |u, _| [u, 0.0, 0.0, 1.0]);
        let out = color_ramp(img, &stops, interpolation);
        out.row(0).iter().map(|p| p[0]).collect()
    }

    #[test]
    fn constant_holds_each_stop_up_to_the_next() {
        let out = ramp(RampInterpolation::Constant);
        assert!(out.iter().all(|v| [0.0, 0.5, 1.0].contains(v)), "{out:?}");
        assert_eq!(
            sample_ramp(
                &[stop(0.2, 0.0), stop(0.5, 0.5)],
                RampInterpolation::Constant,
                0.5
            )[0],
            0.5
        );
        assert_eq!(
            sample_ramp(&[stop(0.2, 0.3)], RampInterpolation::Constant, 0.1)[0],
            0.3
        );
    }

    #[test]
    fn ease_meets_linear_at_stops_and_midpoints_and_bends_between() {
        let sorted = [stop(0.2, 0.0), stop(0.6, 1.0)];
        let at = |i, t| sample_ramp(&sorted, i, t)[0];
        for t in [0.0, 0.2, 0.4, 0.6, 1.0] {
            let (lin, ease) = (
                at(RampInterpolation::Linear, t),
                at(RampInterpolation::Ease, t),
            );
            assert!((lin - ease).abs() < 1e-6, "t={t}: {lin} vs {ease}");
        }
        // A quarter of the way: linear 0.25, smoothstep 0.15625.
        assert!((at(RampInterpolation::Linear, 0.3) - 0.25).abs() < 1e-6);
        assert!((at(RampInterpolation::Ease, 0.3) - 0.15625).abs() < 1e-6);
    }

    /// The binary search picks the stop the linear walk does, bit for bit: on
    /// shared positions, at and past the ends, on a NaN or infinite `t`.
    #[test]
    fn the_search_agrees_with_the_walk() {
        let stops: Vec<RampStop> = [0.0, 0.1, 0.1, 0.1, 0.4, 0.4, 0.75, 1.0, 1.0]
            .iter()
            .enumerate()
            .map(|(i, &pos)| stop(pos, i as f32 / 8.0))
            .collect();
        let mut ts: Vec<f32> = (-10..=110).map(|i| i as f32 / 100.0).collect();
        ts.extend([0.1, 0.4, f32::NAN, f32::INFINITY, -f32::INFINITY]);
        for interp in [
            RampInterpolation::Constant,
            RampInterpolation::Linear,
            RampInterpolation::Ease,
        ] {
            for n in 1..=stops.len() {
                for &t in &ts {
                    let (got, want) = (
                        sample_ramp(&stops[..n], interp, t),
                        scan_ramp(&stops[..n], interp, t),
                    );
                    assert_eq!(got.map(f32::to_bits), want.map(f32::to_bits), "{n} {t}");
                }
            }
        }
    }
}
