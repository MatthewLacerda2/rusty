//! Lettering (#2190): a string shaped in a font and laid out as one centred block,
//! each glyph's outline a [`Shape`] in tile units.
//!
//! Each line is shaped whole (`rusty_font::shape`: kerning, ligatures, marks, as
//! the UI's `Text` draws them), its glyphs drawn at the em size `size` with
//! `tracking` ems between them. Lines are `line_spacing` ems apart; the block runs
//! from the first line's ascent to the last line's descent, its widest line sets
//! its width, and it is centred in the tile, each line aligned within it.

use glam::Vec2;
use rusty_core::collections::Map;
use rusty_font::{FontData, GlyphId};

use super::fill::Shape;
use crate::catalog::generators::outline::text::Align;

/// How a block of text is laid out: the em size and tracking in tiles and ems,
/// line spacing in ems, and the alignment of its lines.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub size: f32,
    pub tracking: f32,
    pub line_spacing: f32,
    pub align: Align,
}

/// `text`'s glyphs in `font` under `layout`, one [`Shape`] per drawn glyph.
pub fn glyphs(font: &FontData, text: &str, layout: Layout) -> Vec<Shape> {
    let upem = font.units_per_em();
    let s = layout.size / upem;
    let lines: Vec<Line> = text
        .split('\n')
        .map(|l| {
            line(
                font,
                l.strip_suffix('\r').unwrap_or(l),
                layout.tracking * upem,
            )
        })
        .collect();
    let block = lines.iter().map(|l| l.1).fold(0.0, f32::max) * s;
    let left = 0.5 - block / 2.0;
    let [ascent, descent, _] = font.vertical_metrics(layout.size);
    let pitch = layout.size * layout.line_spacing;
    let height = ascent + descent + pitch * (lines.len() - 1) as f32;
    let top = 0.5 - height / 2.0;
    // Each distinct glyph is flattened once, however often the text draws it.
    let mut outlines: Map<GlyphId, Option<Vec<rusty_font::Segment>>> = Map::default();
    let mut shapes = Vec::new();
    for (i, (glyphs, w)) in lines.iter().enumerate() {
        let x = match layout.align {
            Align::Left => left,
            Align::Center => 0.5 - w * s / 2.0,
            Align::Right => left + block - w * s,
        };
        let origin = Vec2::new(x, top + ascent + pitch * i as f32);
        for &(glyph, pen) in glyphs {
            let outline = outlines
                .entry(glyph)
                .or_insert_with(|| font.outline(glyph, upem));
            let Some(outline) = outline else {
                continue;
            };
            let place = |p: Vec2| origin + Vec2::new(pen.x + p.x, -(pen.y + p.y)) * s;
            shapes.push(
                outline
                    .iter()
                    .map(|[a, b]| [place(*a), place(*b)])
                    .collect(),
            );
        }
    }
    shapes
}

/// One drawn line: each glyph and where it sits (font units, y up), and the line's
/// advance width.
type Line = (Vec<(GlyphId, Vec2)>, f32);

/// `text` shaped as one line, `tracking` font units between glyphs that advance.
fn line(font: &FontData, text: &str, tracking: f32) -> Line {
    let mut pen = 0.0;
    let shaped = rusty_font::shape(font, text, &[]);
    let mut out = Vec::with_capacity(shaped.len());
    for (i, g) in shaped.iter().enumerate() {
        out.push((g.glyph, Vec2::new(pen, 0.0) + g.offset));
        pen += g.advance;
        if i + 1 < shaped.len() && g.advance != 0.0 {
            pen += tracking;
        }
    }
    (out, pen)
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
