//! src/render/ui/text/sdf.rs — one glyph's signed distance field (#419).
//!
//! The glyph's flattened outline (`FontData::outline`) is filled at
//! [`SUPERSAMPLE`]× the atlas base size — non-zero winding, sampled at each cell's
//! centre, so a cell is inside or outside — and run through an exact Euclidean distance transform
//! (Felzenszwalb–Huttenlocher) both ways; each base-size texel averages the signed
//! distances of its supersampled block. Distances are stored as
//! `0.5 + d / (2 · SPREAD)` (inside positive), so the edge is 0.5 and the field
//! reaches [`SPREAD`] base pixels past it — the room outline, glow and faux bold
//! have to grow into. Pure CPU.

use glam::Vec2;

use crate::ui::text::font::{FontData, GlyphId};

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
    let s = SUPERSAMPLE as f32;
    let segments = font.outline(glyph, BASE_SIZE * s)?;
    let (lo, hi) = segments.iter().flatten().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    if !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    let pad = SPREAD as i32;
    // The base grid, y up: `(x0, y0)` is its bottom-left corner relative to the pen.
    let (x0, y0) = (
        (lo.x / s).floor() as i32 - pad,
        (lo.y / s).floor() as i32 - pad,
    );
    let (x1, y1) = (
        (hi.x / s).ceil() as i32 + pad,
        (hi.y / s).ceil() as i32 + pad,
    );
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let corner = Vec2::new(x0 as f32, y1 as f32) * s;
    let inside = fill(&segments, corner, w * SUPERSAMPLE, h * SUPERSAMPLE);
    let signed = signed_distance(&inside, w * SUPERSAMPLE, h * SUPERSAMPLE);
    let pixels = downsample(&signed, w, h);
    Some(GlyphField {
        width: w as u32,
        height: h as u32,
        pixels,
        origin: Vec2::new(x0 as f32, y0 as f32),
    })
}

/// The non-zero-winding inside mask of `segments` over a `(w, h)` cell grid whose
/// top-left corner is `corner` (y up; rows top first), sampled at cell centres.
fn fill(segments: &[[Vec2; 2]], corner: Vec2, w: usize, h: usize) -> Vec<bool> {
    let mut inside = vec![false; w * h];
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    for row in 0..h {
        let y = corner.y - row as f32 - 0.5;
        crossings.clear();
        for [a, b] in segments {
            let (up, down) = (a.y <= y && y < b.y, b.y <= y && y < a.y);
            if up || down {
                let x = a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x);
                crossings.push((x, if up { 1 } else { -1 }));
            }
        }
        crossings.sort_by(|p, q| p.0.total_cmp(&q.0));
        let mut winding = 0;
        for pair in crossings.windows(2) {
            winding += pair[0].1;
            if winding == 0 {
                continue;
            }
            // Cells whose centre lies in [pair[0].x, pair[1].x).
            let from = (pair[0].0 - corner.x - 0.5).ceil().max(0.0) as usize;
            let to = ((pair[1].0 - corner.x - 0.5).ceil().max(0.0) as usize).min(w);
            for cell in &mut inside[row * w + from.min(w)..row * w + to] {
                *cell = true;
            }
        }
    }
    inside
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
        let wgsl = include_str!("../../../../engine/shaders/ui.wgsl");
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
