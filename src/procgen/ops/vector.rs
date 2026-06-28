//! src/procgen/ops/vector.rs — vector / normal ops.
//!
//! These reshape the *domain* or reinterpret channels: [`mapping`] resamples the
//! input under an affine domain transform (with wrap, so it stays tiling),
//! [`bump_to_normal`] bakes a tangent-space normal map from a height field, and the
//! combine/separate pair shuttle between grayscale channels and an RGB image.

use super::super::image_buf::{Image, Rgba};
use super::math::luminance;

/// Resample `img` under an affine domain transform: translate, rotate (turns) about
/// the domain center, then scale. Sampling wraps (nearest, modular), so the output
/// still tiles. A `scale > 1` packs more repeats into the canvas.
pub fn mapping(img: &Image, scale: [f32; 2], rotation: f32, translation: [f32; 2]) -> Image {
    let res = img.resolution();
    let (sx, sy) = (scale[0].abs().max(1e-4), scale[1].abs().max(1e-4));
    let ang = rotation * std::f32::consts::TAU;
    let (ca, sa) = (ang.cos(), ang.sin());
    Image::fill_uv(res, |u, v| {
        // Center the domain so rotation/scale pivot at (0.5, 0.5).
        let (cx, cy) = (u - 0.5, v - 0.5);
        let rx = cx * ca - cy * sa;
        let ry = cx * sa + cy * ca;
        let su = rx * sx + 0.5 + translation[0];
        let sv = ry * sy + 0.5 + translation[1];
        // Wrapped nearest fetch keeps the result seamless.
        let px = (su * res as f32).floor() as i64;
        let py = (sv * res as f32).floor() as i64;
        img.get_wrapped(px, py)
    })
}

/// Bake a tangent-space normal map from `img`'s red channel as a height field. Uses
/// central differences (wrapped, so the normals tile) and packs the unit normal into
/// `[0, 1]` RGB the renderer's `2*n - 1` decode expects, with B≈1 for a flat surface.
pub fn bump_to_normal(img: &Image, strength: f32) -> Image {
    let res = img.resolution();
    let h = |x: i64, y: i64| img.get_wrapped(x, y)[0];
    Image::fill_uv(res, |u, v| {
        let x = (u * res as f32) as i64;
        let y = (v * res as f32) as i64;
        // dz/dx and dz/dy via central differences; strength scales the slope.
        let dx = (h(x + 1, y) - h(x - 1, y)) * 0.5 * strength;
        let dy = (h(x, y + 1) - h(x, y - 1)) * 0.5 * strength;
        // Tangent-space normal of the height surface: (-dx, -dy, 1) normalized.
        let nx = -dx;
        let ny = -dy;
        let nz = 1.0;
        let inv = 1.0 / (nx * nx + ny * ny + nz * nz).sqrt();
        [
            nx * inv * 0.5 + 0.5,
            ny * inv * 0.5 + 0.5,
            nz * inv * 0.5 + 0.5,
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
