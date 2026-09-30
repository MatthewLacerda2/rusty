//! src/procgen/ops/vector.rs — vector / normal ops.
//!
//! These reshape the *domain* or reinterpret channels: [`mapping`] resamples the
//! input under an affine domain transform (with wrap, so it stays tiling),
//! [`bump_to_normal`] bakes a tangent-space normal map from a height field, and the
//! combine/separate pair shuttle between grayscale channels and an RGB image.

use super::super::image_buf::{Image, Rgba};
use super::generators::count;
use super::math::luminance;

/// Resample `img` under an affine domain transform: translate, rotate (turns) about
/// the domain center, then scale; a `scale > 1` packs more repeats into the canvas.
/// Sampling is bilinear and wraps. With `tiling`, `scale` rounds to whole repeats and
/// `rotation` snaps to quarter turns — the only transforms that map the unit tile's
/// lattice onto itself, so the output still tiles (#392).
pub fn mapping(
    img: &Image,
    scale: [f32; 2],
    rotation: f32,
    translation: [f32; 2],
    tiling: bool,
) -> Image {
    let [su, sv] = scale.map(|s| s.abs().max(1e-4));
    let (sx, sy, (ca, sa)) = if tiling {
        (count(su), count(sv), quarter_turn(rotation))
    } else {
        let ang = rotation * std::f32::consts::TAU;
        (su, sv, (ang.cos(), ang.sin()))
    };
    Image::fill_uv(img.resolution(), |u, v| {
        // Center the domain so rotation/scale pivot at (0.5, 0.5).
        let (cx, cy) = (u - 0.5, v - 0.5);
        let rx = cx * ca - cy * sa;
        let ry = cx * sa + cy * ca;
        img.sample_bilinear_wrapped(
            rx * sx + 0.5 + translation[0],
            ry * sy + 0.5 + translation[1],
        )
    })
}

/// `(cos, sin)` of `turns` snapped to the nearest quarter turn — exact, not via
/// `f32` trig, so a snapped rotation adds no rounding drift at the wrap.
fn quarter_turn(turns: f32) -> (f32, f32) {
    match ((turns * 4.0).round() as i64).rem_euclid(4) {
        0 => (1.0, 0.0),
        1 => (0.0, 1.0),
        2 => (-1.0, 0.0),
        _ => (0.0, -1.0),
    }
}

/// Bake a tangent-space normal map from `img`'s red channel as a height field. Uses
/// central differences (wrapped, so the normals tile) converted to **tile units** —
/// the per-pixel delta times the resolution — so `strength` means the same slope at
/// any resolution (#394). Packs the unit normal into `[0, 1]` RGB the renderer's
/// `2*n - 1` decode expects, with B≈1 for a flat surface.
pub fn bump_to_normal(img: &Image, strength: f32) -> Image {
    let res = img.resolution();
    let h = |x: i64, y: i64| img.get_wrapped(x, y)[0];
    // Central difference spans two pixels = 2/res of a tile.
    let k = 0.5 * res as f32 * strength;
    Image::fill_uv(res, |u, v| {
        let x = (u * res as f32) as i64;
        let y = (v * res as f32) as i64;
        let dx = (h(x + 1, y) - h(x - 1, y)) * k;
        let dy = (h(x, y + 1) - h(x, y - 1)) * k;
        // Tangent-space normal of the height surface: (-dx, -dy, 1) normalized.
        let inv = 1.0 / (dx * dx + dy * dy + 1.0).sqrt();
        [
            -dx * inv * 0.5 + 0.5,
            -dy * inv * 0.5 + 0.5,
            inv * 0.5 + 0.5,
            1.0,
        ]
    })
}

/// Combine up to three single-channel inputs into one RGB image (alpha 1). Each input
/// contributes its red channel to one output channel; missing inputs contribute 0.
pub fn combine_rgb(inputs: &[&Image], resolution: u32) -> Image {
    let mut out = Image::new(resolution);
    let n = out.pixels().len();
    let mut buf = vec![[0.0f32, 0.0, 0.0, 1.0]; n];
    for (c, src) in inputs.iter().take(3).enumerate() {
        let sp = src.pixels();
        for (i, b) in buf.iter_mut().enumerate() {
            b[c] = sp[i][0];
        }
    }
    out.pixels_mut().copy_from_slice(&buf);
    out
}

/// Pull one channel of `img` out as grayscale (written to all RGB). `channel`
/// 0=R, 1=G, 2=B, 3=A (alpha→gray, output alpha forced to 1).
pub fn separate_rgb(mut img: Image, channel: u8) -> Image {
    let c = (channel as usize).min(3);
    img.map_in_place(|p: Rgba| {
        let v = p[c];
        [v, v, v, 1.0]
    });
    img
}

/// Treat an RGB pixel as a height by its luminance — used when a height-consuming op
/// is fed a color image rather than a grayscale one.
#[inline]
pub fn height_of(p: Rgba) -> f32 {
    luminance(p)
}
