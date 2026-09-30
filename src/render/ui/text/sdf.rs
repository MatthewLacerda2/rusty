//! src/render/ui/text/sdf.rs — one glyph's signed distance field (#419).
//!
//! The glyph is rasterized at [`SUPERSAMPLE`]× the atlas base size, thresholded to
//! inside / outside, and run through an exact Euclidean distance transform
//! (Felzenszwalb–Huttenlocher) both ways; each base-size texel averages the signed
//! distances of its supersampled block. Distances are stored as
//! `0.5 + d / (2 · SPREAD)` (inside positive), so the edge is 0.5 and the field
//! reaches [`SPREAD`] base pixels past it — the room outline, glow and faux bold
//! have to grow into. Pure CPU.

use ab_glyph::{point, Font, GlyphId};
use glam::Vec2;

use crate::ui::text::font::FontData;

/// The em size glyphs are generated at, in atlas pixels.
pub(crate) const BASE_SIZE: f32 = 48.0;
/// How far the field reaches past the edge, in atlas pixels (also each glyph's
/// padding). An outline + glow + bold reach of up to `SPREAD / BASE_SIZE` ems fits.
pub(crate) const SPREAD: f32 = 12.0;
/// Supersampling factor of the coverage raster the field is measured on.
const SUPERSAMPLE: usize = 4;

/// One glyph's field.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GlyphField {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Row-major, top row first.
    pub(crate) pixels: Vec<u8>,
    /// The bitmap's bottom-left corner relative to the pen, base pixels, y up.
    pub(crate) origin: Vec2,
}

/// The field of `glyph` in `font`, or `None` for an empty glyph (a space).
pub(crate) fn glyph_field(font: &FontData, glyph: GlyphId) -> Option<GlyphField> {
    let base = font.em_scale(BASE_SIZE);
    let pad = SPREAD as i32;
    let bounds = font
        .font()
        .outline_glyph(glyph.with_scale_and_position(base, point(0.0, 0.0)))?
        .px_bounds();
    // The base grid, y down: `(x0, y0)` is its top-left pixel relative to the pen.
    let (x0, y0) = (
        bounds.min.x.floor() as i32 - pad,
        bounds.min.y.floor() as i32 - pad,
    );
    let w = (bounds.max.x.ceil() as i32 - bounds.min.x.floor() as i32 + 2 * pad) as usize;
    let h = (bounds.max.y.ceil() as i32 - bounds.min.y.floor() as i32 + 2 * pad) as usize;
    let inside = coverage(font, glyph, (x0, y0), (w, h))?;
    let signed = signed_distance(&inside, w * SUPERSAMPLE, h * SUPERSAMPLE);
    let pixels = downsample(&signed, w, h);
    Some(GlyphField {
        width: w as u32,
        height: h as u32,
        pixels,
        origin: Vec2::new(x0 as f32, -(y0 + h as i32) as f32),
    })
}

/// The supersampled inside mask over the `(w, h)` base grid at `(x0, y0)`.
fn coverage(
    font: &FontData,
    glyph: GlyphId,
    (x0, y0): (i32, i32),
    (w, h): (usize, usize),
) -> Option<Vec<bool>> {
    let s = SUPERSAMPLE as f32;
    let scale = font.em_scale(BASE_SIZE * s);
    let outline = font
        .font()
        .outline_glyph(glyph.with_scale_and_position(scale, point(0.0, 0.0)))?;
    let min = outline.px_bounds().min;
    let (sw, sh) = (w * SUPERSAMPLE, h * SUPERSAMPLE);
    let mut inside = vec![false; sw * sh];
    let (ox, oy) = (
        min.x as i32 - x0 * SUPERSAMPLE as i32,
        min.y as i32 - y0 * SUPERSAMPLE as i32,
    );
    outline.draw(|x, y, c| {
        let (gx, gy) = (x as i32 + ox, y as i32 + oy);
        if c >= 0.5 && gx >= 0 && gy >= 0 && (gx as usize) < sw && (gy as usize) < sh {
            inside[gy as usize * sw + gx as usize] = true;
        }
    });
    Some(inside)
}

/// Signed distance per supersampled cell, in supersampled cells: positive inside,
/// the edge halfway between an inside and an outside cell centre.
fn signed_distance(inside: &[bool], w: usize, h: usize) -> Vec<f32> {
    let to_inside = edt(inside, w, h, true);
    let to_outside = edt(inside, w, h, false);
    inside
        .iter()
        .zip(to_inside.iter().zip(&to_outside))
        .map(|(&i, (&di, &dout))| {
            if i {
                dout.sqrt() - 0.5
            } else {
                0.5 - di.sqrt()
            }
        })
        .collect()
}

/// Average each base texel's supersampled block and encode it.
fn downsample(signed: &[f32], w: usize, h: usize) -> Vec<u8> {
    let sw = w * SUPERSAMPLE;
    let n = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0;
            for sy in 0..SUPERSAMPLE {
                let row = (y * SUPERSAMPLE + sy) * sw + x * SUPERSAMPLE;
                sum += signed[row..row + SUPERSAMPLE].iter().sum::<f32>();
            }
            let d = sum / n / SUPERSAMPLE as f32;
            let v = (0.5 + d / (2.0 * SPREAD)).clamp(0.0, 1.0);
            out.push((v * 255.0).round() as u8);
        }
    }
    out
}

/// Squared distance from each cell to the nearest cell whose mask equals `target`.
fn edt(mask: &[bool], w: usize, h: usize, target: bool) -> Vec<f32> {
    const FAR: f32 = 1e20;
    let mut grid: Vec<f32> = mask
        .iter()
        .map(|&m| if m == target { 0.0 } else { FAR })
        .collect();
    let mut buf = vec![0.0; w.max(h)];
    for x in 0..w {
        for y in 0..h {
            buf[y] = grid[y * w + x];
        }
        let col = edt_1d(&buf[..h]);
        for y in 0..h {
            grid[y * w + x] = col[y];
        }
    }
    for y in 0..h {
        let row = edt_1d(&grid[y * w..(y + 1) * w]);
        grid[y * w..(y + 1) * w].copy_from_slice(&row);
    }
    grid
}

/// The 1D squared distance transform of sampled function `f` (lower envelope of
/// parabolas).
fn edt_1d(f: &[f32]) -> Vec<f32> {
    let n = f.len();
    let mut d = vec![0.0; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];
    let mut k = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    let parabola = |q: usize, p: usize| {
        ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q - p) as f32)
    };
    for q in 1..n {
        let mut s = parabola(q, v[k]);
        while s <= z[k] {
            k -= 1; // z[0] is -inf, so this stops at the first parabola
            s = parabola(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    let mut k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f32 - p as f32;
        *out = dq * dq + f[p];
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::text::font;

    #[test]
    fn the_shader_decodes_with_the_same_spread() {
        let wgsl = include_str!("../../../../assets/shaders/ui.wgsl");
        assert!(wgsl.contains(&format!("const SPREAD: f32 = {SPREAD:.1};")));
    }

    #[test]
    fn the_1d_transform_measures_to_the_nearest_zero() {
        let f = [1e20, 0.0, 1e20, 1e20, 1e20, 0.0];
        assert_eq!(edt_1d(&f), vec![1.0, 0.0, 1.0, 4.0, 1.0, 0.0]);
    }

    #[test]
    fn a_glyph_field_is_inside_at_its_stroke_and_outside_at_its_padding() {
        let f = font::default_font();
        let field = glyph_field(&f, f.glyph('I').expect("I")).expect("I has an outline");
        let (w, h) = (field.width as usize, field.height as usize);
        let at = |x: usize, y: usize| field.pixels[y * w + x];
        assert!(at(w / 2, h / 2) > 128, "the stem's centre is inside");
        assert_eq!(at(0, 0), 0, "a corner of the padding is past the spread");
        assert!(
            at(w / 2, SPREAD as usize / 2) < 128,
            "above the stem is outside"
        );
        assert!(
            field.origin.x < 0.0 && field.origin.y < 0.0,
            "padding sits left/below the pen"
        );
        assert!(glyph_field(&f, f.glyph(' ').expect("space")).is_none());
    }
}
