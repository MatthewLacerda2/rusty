//! src/procgen/ops/color.rs — color-grading ops (one input, or two for Mix).
//!
//! These map pixel→pixel (no neighbourhood), so they run through
//! [`Image::map_in_place`] — except [`mix`], which combines two equal-sized inputs
//! (optionally through a third, the per-pixel mask) row by row
//! ([`Image::for_each_row`]). Both band rows across threads (#1490). The grading
//! amounts are [`Field`]s, read per pixel, so a link can vary them (#1494).
//! All work in unclamped linear float; the bake clamps once at the end.

use super::super::image_buf::{Image, Rgba};
use super::super::recipe::{BlendMode, RampInterpolation, RampStop};
use super::field::Field;

/// Invert RGB (`1 - c`); alpha is left as-is.
pub fn invert(mut img: Image) -> Image {
    img.map_in_place(|p| [1.0 - p[0], 1.0 - p[1], 1.0 - p[2], p[3]]);
    img
}

/// Per-channel gamma (`c^gamma`) on RGB; alpha untouched.
pub fn gamma(mut img: Image, g: Field) -> Image {
    img.map_indexed(|i, p| {
        let g = g.at(i).max(1e-4);
        [
            p[0].max(0.0).powf(g),
            p[1].max(0.0).powf(g),
            p[2].max(0.0).powf(g),
            p[3],
        ]
    });
    img
}

/// Stretch RGB so the input's darkest channel value maps to 0 and its brightest to 1:
/// one range across R, G and B, so a colour keeps its hue (Substance's Auto Levels).
/// Alpha is untouched; a flat input (no range to stretch) passes through unchanged.
pub fn auto_levels(mut img: Image) -> Image {
    let (lo, hi) = img
        .pixels()
        .iter()
        .flat_map(|p| &p[..3])
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &c| {
            (lo.min(c), hi.max(c))
        });
    let span = hi - lo;
    if span.is_finite() && span > 1e-6 {
        let f = |c: f32| (c - lo) / span;
        img.map_in_place(|p| [f(p[0]), f(p[1]), f(p[2]), p[3]]);
    }
    img
}

/// Brightness offset, then contrast scaled about mid-gray (0.5).
pub fn bright_contrast(mut img: Image, bright: Field, contrast: Field) -> Image {
    img.map_indexed(|i, p| {
        let (bright, contrast) = (bright.at(i), contrast.at(i));
        let adj = |c: f32| (c + bright - 0.5) * contrast + 0.5;
        [adj(p[0]), adj(p[1]), adj(p[2]), p[3]]
    });
    img
}

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
/// `linear` and `ease` blend the bracketing stops, `ease` through a smoothstep.
fn sample_ramp(sorted: &[RampStop], interpolation: RampInterpolation, t: f32) -> Rgba {
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
    for w in sorted.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t >= a.pos && t <= b.pos {
            let span = (b.pos - a.pos).max(1e-6);
            let f = (t - a.pos) / span;
            let f = if ease { f * f * (3.0 - 2.0 * f) } else { f };
            return [
                lerp(a.color[0], b.color[0], f),
                lerp(a.color[1], b.color[1], f),
                lerp(a.color[2], b.color[2], f),
                lerp(a.color[3], b.color[3], f),
            ];
        }
    }
    last.color
}

/// Hue rotate (turns), saturation scale, value scale — via RGB→HSV→RGB.
pub fn hue_sat_value(mut img: Image, hue: Field, sat: Field, value: Field) -> Image {
    img.map_indexed(|i, p| {
        let (mut h, mut s, mut v) = rgb_to_hsv(p[0], p[1], p[2]);
        h = (h + hue.at(i)).rem_euclid(1.0);
        s = (s * sat.at(i)).clamp(0.0, 1.0);
        v *= value.at(i);
        let (r, g, b) = hsv_to_rgb(h, s, v);
        [r, g, b, p[3]]
    });
    img
}

/// Blend two equal-sized inputs by `factor` under `mode`. With a `mask`, each
/// pixel's factor is `factor × mask.red` — Blender's Mix with Factor wired to a
/// socket (#405). Differing sizes are a recipe error caught upstream; here `b` and
/// `mask` are sampled positionally.
pub fn mix(a: Image, b: &Image, mask: Option<&Image>, mode: BlendMode, factor: f32) -> Image {
    let mut out = a;
    let w = out.width() as usize;
    out.for_each_row(|y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            let i = y * w + x;
            let f = mask.map_or(factor, |m| factor * m.pixels()[i][0]);
            *p = blend_pixel(*p, b.pixels()[i], mode, f);
        }
    });
    out
}

/// Blend one pair of pixels: compute the mode's combination, then lerp from `a` to it
/// by `factor` (so `factor = 0` is a no-op, the Blender convention).
fn blend_pixel(a: Rgba, b: Rgba, mode: BlendMode, factor: f32) -> Rgba {
    let f = factor.clamp(0.0, 1.0);
    let mut out = a;
    for c in 0..3 {
        let combined = match mode {
            BlendMode::Mix => b[c],
            BlendMode::Add => a[c] + b[c],
            BlendMode::Subtract => a[c] - b[c],
            BlendMode::Multiply => a[c] * b[c],
            BlendMode::Screen => 1.0 - (1.0 - a[c]) * (1.0 - b[c]),
            BlendMode::Difference => (a[c] - b[c]).abs(),
            BlendMode::Overlay => overlay(a[c], b[c]),
            BlendMode::Lighten => a[c].max(b[c]),
            BlendMode::Darken => a[c].min(b[c]),
            BlendMode::Divide if b[c].abs() < 1e-9 => a[c],
            BlendMode::Divide => a[c] / b[c],
        };
        out[c] = lerp(a[c], combined, f);
    }
    out
}

/// Photoshop "overlay": multiply on the dark half, screen on the bright half.
fn overlay(base: f32, top: f32) -> f32 {
    if base < 0.5 {
        2.0 * base * top
    } else {
        1.0 - 2.0 * (1.0 - base) * (1.0 - top)
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// RGB→HSV, all in `[0, 1]` (hue as a fraction of a turn).
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let v = max;
    let s = if max <= 0.0 { 0.0 } else { d / max };
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, v)
}

/// HSV→RGB, inverse of [`rgb_to_hsv`].
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h6 = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h6.rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
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

    #[test]
    fn auto_levels_leaves_a_range_at_its_flat_threshold_alone() {
        // A span of exactly 1e-6 is still "flat"; just past it stretches.
        let at = |hi: f32| {
            let mut img = Image::filled(2, [0.0, 0.0, 0.0, 1.0]);
            img.pixels_mut()[0] = [hi, hi, hi, 1.0];
            auto_levels(img).pixels()[0][0]
        };
        assert_eq!(at(1e-6), 1e-6);
        assert_eq!(at(2e-6), 1.0);
    }
}
