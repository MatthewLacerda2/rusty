//! src/procgen/ops/filter.rs — neighbourhood ops.
//!
//! A small **separable box blur**: average a `(2r+1)` window horizontally, then
//! vertically. Two 1-D passes cost `O(n·r)` instead of `O(n·r²)`, and the window
//! wraps at the edges (via [`Image::get_wrapped`]) so the blurred result stays tiling
//! — its main use is softening a height field before [`super::vector::bump_to_normal`]
//! and feathering masks.
//!
//! [`cavity`] and [`curvature`] derive weathering masks from a height field (its red
//! channel) the way Substance does (#408): both wrap, and both take their reach in
//! tile units, so the masks tile and look the same at any resolution.

use super::super::image_buf::{Image, Rgba};

/// A blur radius given as a fraction of the tile width, in pixels at `resolution`:
/// `round(radius * resolution)`, at most half the tile (a wider window just wraps
/// back over pixels it already averaged). Negative or NaN reads as no blur.
pub fn pixel_radius(radius: f32, resolution: u32) -> u32 {
    let px = (radius * resolution as f32).round();
    if px >= 1.0 {
        (px as u32).min(resolution / 2)
    } else {
        0
    }
}

/// Separable box blur of `radius` **pixels** (see [`pixel_radius`] for the recipe's
/// tile-fraction unit). `radius == 0` returns the input unchanged.
pub fn blur(img: &Image, radius: u32) -> Image {
    if radius == 0 {
        return img.clone();
    }
    let res = img.resolution();
    let r = radius as i64;
    let inv = 1.0 / (2 * radius + 1) as f32;
    // Horizontal pass into a temp, then vertical pass into the output.
    let horizontal = blur_axis(img, res, r, inv, true);
    blur_axis(&horizontal, res, r, inv, false)
}

/// One 1-D box-average pass along x (`horizontal`) or y, wrapping at the edges.
fn blur_axis(src: &Image, res: u32, r: i64, inv: f32, horizontal: bool) -> Image {
    let mut out = Image::new(res);
    let res_i = res as i64;
    for y in 0..res_i {
        for x in 0..res_i {
            let mut acc: Rgba = [0.0, 0.0, 0.0, 0.0];
            for k in -r..=r {
                let p = if horizontal {
                    src.get_wrapped(x + k, y)
                } else {
                    src.get_wrapped(x, y + k)
                };
                for c in 0..4 {
                    acc[c] += p[c];
                }
            }
            let idx = (y * res_i + x) as usize;
            out.pixels_mut()[idx] = [acc[0] * inv, acc[1] * inv, acc[2] * inv, acc[3] * inv];
        }
    }
    out
}

/// Cavity mask: how far each pixel sits below its `radius`-pixel neighbourhood,
/// `1 − max(blur(h) − h, 0)`, as grayscale. Flat and raised ground stay white;
/// crevices go dark — multiply it into albedo for baked occlusion, or invert it for
/// a dirt mask.
pub fn cavity(img: &Image, radius: u32) -> Image {
    let around = blur(img, radius);
    let mut out = img.clone();
    for (p, b) in out.pixels_mut().iter_mut().zip(around.pixels()) {
        let v = 1.0 - (b[0] - p[0]).max(0.0);
        *p = [v, v, v, 1.0];
    }
    out
}

/// Curvature: `0.5 + strength × (h − ring mean)`, where the ring is eight bilinear,
/// wrapped samples `radius` (a tile fraction) away — a discrete Laplacian at that
/// scale. Above mid-grey is convex (raised edges: wear), below is concave (grime).
/// Bilinear taps keep sub-pixel radii smooth, so it holds across resolutions.
pub fn curvature(img: &Image, radius: f32, strength: f32) -> Image {
    let h = |u: f32, v: f32| img.sample_bilinear_wrapped(u, v)[0];
    let diag = radius * std::f32::consts::FRAC_1_SQRT_2;
    let ring = [
        (radius, 0.0),
        (-radius, 0.0),
        (0.0, radius),
        (0.0, -radius),
        (diag, diag),
        (diag, -diag),
        (-diag, diag),
        (-diag, -diag),
    ];
    let res = img.resolution();
    Image::fill_uv(res, |u, v| {
        let mean = ring.iter().map(|(du, dv)| h(u + du, v + dv)).sum::<f32>() / 8.0;
        let c = 0.5 + strength * (h(u, v) - mean);
        [c, c, c, 1.0]
    })
}
