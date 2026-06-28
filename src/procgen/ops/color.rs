//! src/procgen/ops/color.rs — color-grading ops (one input, or two for Mix).
//!
//! These map pixel→pixel (no neighbourhood), so they run through
//! [`Image::map_in_place`] — except [`mix`], which combines two equal-sized inputs.
//! All work in unclamped linear float; the bake clamps once at the end.

use super::super::image_buf::{Image, Rgba};
use super::super::recipe::{BlendMode, RampStop};

/// Invert RGB (`1 - c`); alpha is left as-is.
pub fn invert(mut img: Image) -> Image {
    img.map_in_place(|p| [1.0 - p[0], 1.0 - p[1], 1.0 - p[2], p[3]]);
    img
}

/// Per-channel gamma (`c^gamma`) on RGB; alpha untouched.
pub fn gamma(mut img: Image, g: f32) -> Image {
    let g = g.max(1e-4);
    img.map_in_place(|p| {
        [
            p[0].max(0.0).powf(g),
            p[1].max(0.0).powf(g),
            p[2].max(0.0).powf(g),
            p[3],
        ]
    });
    img
}

/// Brightness offset, then contrast scaled about mid-gray (0.5).
pub fn bright_contrast(mut img: Image, bright: f32, contrast: f32) -> Image {
    let adj = |c: f32| (c + bright - 0.5) * contrast + 0.5;
    img.map_in_place(|p| [adj(p[0]), adj(p[1]), adj(p[2]), p[3]]);
    img
}

/// Map the input's red channel through a sorted gradient of [`RampStop`]s. With no
/// stops the input passes through unchanged.
pub fn color_ramp(mut img: Image, stops: &[RampStop]) -> Image {
    if stops.is_empty() {
        return img;
    }
    let mut sorted = stops.to_vec();
    sorted.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    img.map_in_place(|p| sample_ramp(&sorted, p[0]));
    img
}

/// Sample a sorted ramp at scalar `t`, linearly interpolating between bracketing
/// stops and clamping to the ends.
fn sample_ramp(sorted: &[RampStop], t: f32) -> Rgba {
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
pub fn hue_sat_value(mut img: Image, hue: f32, sat: f32, value: f32) -> Image {
    img.map_in_place(|p| {
        let (mut h, mut s, mut v) = rgb_to_hsv(p[0], p[1], p[2]);
        h = (h + hue).rem_euclid(1.0);
        s = (s * sat).clamp(0.0, 1.0);
        v *= value;
        let (r, g, b) = hsv_to_rgb(h, s, v);
        [r, g, b, p[3]]
    });
    img
}

/// Blend two equal-sized inputs by `factor` under `mode`. Differing sizes are a
/// recipe error caught upstream; here `b` is sampled positionally.
pub fn mix(a: Image, b: &Image, mode: BlendMode, factor: f32) -> Image {
    let mut out = a;
    let bp = b.pixels();
    for (i, p) in out.pixels_mut().iter_mut().enumerate() {
        let q = bp[i];
        *p = blend_pixel(*p, q, mode, factor);
    }
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
