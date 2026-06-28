//! src/procgen/ops/filter.rs — neighbourhood ops.
//!
//! A small **separable box blur**: average a `(2r+1)` window horizontally, then
//! vertically. Two 1-D passes cost `O(n·r)` instead of `O(n·r²)`, and the window
//! wraps at the edges (via [`Image::get_wrapped`]) so the blurred result stays tiling
//! — its main use is softening a height field before [`super::vector::bump_to_normal`]
//! and feathering masks.

use super::super::image_buf::{Image, Rgba};

/// Separable box blur of `radius` pixels. `radius == 0` returns the input unchanged.
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
