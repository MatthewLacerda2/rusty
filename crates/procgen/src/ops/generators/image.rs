use super::super::super::image_buf::{Image, Size};
use super::super::super::recipe::ImageEncoding;
use super::super::super::source::Source;
use rusty_core::color::gamma_to_linear;

/// sRGB decodes per tap, before the bilinear blend, so the blend is in linear light.
pub fn image(source: &Source, encoding: ImageEncoding, size: Size) -> Image {
    let (w, h) = source.size();
    let texels = source.texels();
    let at = |x: i64, y: i64| {
        let (x, y) = (x.rem_euclid(w as i64), y.rem_euclid(h as i64));
        let [r, g, b, a] = texels[y as usize * w as usize + x as usize];
        match encoding {
            ImageEncoding::Linear => [r, g, b, a],
            ImageEncoding::Srgb => [
                gamma_to_linear(r),
                gamma_to_linear(g),
                gamma_to_linear(b),
                a,
            ],
        }
    };
    let mut img = Image::new(size);
    img.for_each_row(|py, row| {
        let (yi, ty) = tap(py, h, size.height);
        for (px, p) in row.iter_mut().enumerate() {
            let (xi, tx) = tap(px, w, size.width);
            let (p00, p10, p01, p11) = (
                at(xi, yi),
                at(xi + 1, yi),
                at(xi, yi + 1),
                at(xi + 1, yi + 1),
            );
            *p = std::array::from_fn(|c| {
                let top = p00[c] + (p10[c] - p00[c]) * tx;
                let bottom = p01[c] + (p11[c] - p01[c]) * tx;
                top + (bottom - top) * ty
            });
        }
    });
    img
}

/// Texel `(i + ½)·source/out − ½` in integers, so a matching side lands on texel `i` with a
/// fraction of exactly 0, where `f32` lands a hair short.
fn tap(i: usize, source: u32, out: u32) -> (i64, f32) {
    let num = (2 * i as i64 + 1) * source as i64 - out as i64;
    let den = 2 * out as i64;
    (num.div_euclid(den), num.rem_euclid(den) as f32 / den as f32)
}

#[cfg(test)]
mod tests {
    use super::super::super::super::image_buf::Rgba;
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
        let img = image(&src, ImageEncoding::Linear, 4.into());
        assert_eq!(img.pixels(), src.texels());
    }

    #[test]
    fn a_matching_size_copies_every_texel_at_sizes_float_maths_got_wrong() {
        let src = source(100, 100, |x, y| {
            [x as f32 / 99.0, y as f32 / 99.0, 0.25, 1.0]
        });
        let img = image(&src, ImageEncoding::Linear, 100.into());
        assert_eq!(img.pixels(), src.texels());
    }

    #[test]
    fn srgb_decodes_colour_and_leaves_alpha() {
        let src = source(1, 1, |_, _| [0.5, 1.0, 0.0, 0.5]);
        let p = image(&src, ImageEncoding::Srgb, 2.into()).pixels()[0];
        assert_eq!(p, [gamma_to_linear(0.5), 1.0, 0.0, 0.5]);
    }

    #[test]
    fn a_non_square_source_wraps_on_both_axes() {
        let src = source(2, 1, |x, _| [x as f32, 0.0, 0.0, 1.0]);
        let img = image(&src, ImageEncoding::Linear, 4.into());
        let row: Vec<f32> = img.row(0).iter().map(|p| p[0]).collect();
        assert_eq!(row, [0.25, 0.25, 0.75, 0.75]);
    }

    #[test]
    fn a_non_square_extent_follows_the_source_per_axis() {
        let src = source(4, 2, |x, y| [x as f32, y as f32, 0.0, 1.0]);
        let same = image(&src, ImageEncoding::Linear, Size::new(4, 2));
        assert_eq!(same.pixels(), src.texels());
        let wide = image(&src, ImageEncoding::Linear, Size::new(8, 1));
        assert_eq!(wide.size(), Size::new(8, 1));
        assert_eq!(wide.row(0)[2][..2], [0.75, 0.5]);
    }
}
