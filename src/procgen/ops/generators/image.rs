//! The `image` generator (#1491): a decoded file resampled to the recipe's
//! resolution.

use super::super::super::image_buf::{Image, Rgba};
use super::super::super::recipe::ImageEncoding;
use super::super::super::source::Source;
use crate::core::color::gamma_to_linear;

/// `source` read under `encoding` and resampled bilinearly, wrapped, to a
/// `resolution²` image. Each output pixel centre maps to the same point of the
/// source's unit domain, so a source already at `resolution` reads back exactly.
/// sRGB is decoded before filtering, so the blend happens in linear light.
pub fn image(source: &Source, encoding: ImageEncoding, resolution: u32) -> Image {
    let (w, h) = source.size();
    let texels: Vec<Rgba> = match encoding {
        ImageEncoding::Linear => source.texels().to_vec(),
        ImageEncoding::Srgb => source
            .texels()
            .iter()
            .map(|&[r, g, b, a]| {
                [
                    gamma_to_linear(r),
                    gamma_to_linear(g),
                    gamma_to_linear(b),
                    a,
                ]
            })
            .collect(),
    };
    let at = |x: i64, y: i64| {
        let (x, y) = (x.rem_euclid(w as i64), y.rem_euclid(h as i64));
        texels[y as usize * w as usize + x as usize]
    };
    Image::fill_uv(resolution, |u, v| {
        let (x, y) = (u * w as f32 - 0.5, v * h as f32 - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let (p00, p10, p01, p11) = (
            at(xi, yi),
            at(xi + 1, yi),
            at(xi, yi + 1),
            at(xi + 1, yi + 1),
        );
        std::array::from_fn(|c| {
            let top = p00[c] + (p10[c] - p00[c]) * tx;
            let bottom = p01[c] + (p11[c] - p01[c]) * tx;
            top + (bottom - top) * ty
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(w: u32, h: u32, f: impl Fn(u32, u32) -> Rgba) -> Source {
        let texels = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y));
        Source::from_texels(w, h, texels.collect()).unwrap()
    }

    #[test]
    fn a_source_at_the_recipe_resolution_reads_back_exactly() {
        let src = source(4, 4, |x, y| [x as f32 / 3.0, y as f32 / 3.0, 0.5, 1.0]);
        let img = image(&src, ImageEncoding::Linear, 4);
        assert_eq!(img.pixels(), src.texels());
    }

    #[test]
    fn srgb_decodes_colour_and_leaves_alpha() {
        let src = source(1, 1, |_, _| [0.5, 1.0, 0.0, 0.5]);
        let p = image(&src, ImageEncoding::Srgb, 2).pixels()[0];
        assert_eq!(p, [gamma_to_linear(0.5), 1.0, 0.0, 0.5]);
    }

    #[test]
    fn a_non_square_source_wraps_on_both_axes() {
        // 2×1 black|white, upsampled to 4: the column between the white texel and
        // the wrapped black one reads their mean.
        let src = source(2, 1, |x, _| [x as f32, 0.0, 0.0, 1.0]);
        let img = image(&src, ImageEncoding::Linear, 4);
        let row: Vec<f32> = img.row(0).iter().map(|p| p[0]).collect();
        assert_eq!(row, [0.25, 0.25, 0.75, 0.75]);
    }
}
