//! The block runs from the first line's ascent to the last line's descent, its widest line sets
//! its width, and it is centred in the tile.

use glam::Vec2;
use rusty_core::collections::Map;
use rusty_font::{FontData, GlyphId};

use super::fill::Shape;
use crate::catalog::generators::outline::text::Align;

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub size: f32,
    pub tracking: f32,
    pub line_spacing: f32,
    pub align: Align,
}

pub fn glyphs(font: &FontData, text: &str, layout: Layout) -> Vec<Shape> {
    let upem = font.units_per_em();
    let s = layout.size / upem;
    let lines: Vec<Line> = text
        .split('\n')
        .map(|l| line(font, l, layout.tracking * upem))
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
            // A glyph the font lacks advances but stamps no box.
            if glyph == GlyphId::NOTDEF {
                continue;
            }
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

type Line = (Vec<(GlyphId, Vec2)>, f32);

/// `text` shaped as one line, `tracking` font units after each cluster but the
/// last, so a mark keeps its place on its base (as the UI's `Text` spaces). A tab
/// shapes as a space and a stray `\r` is dropped, as the UI's `Text` reads them.
fn line(font: &FontData, text: &str, tracking: f32) -> Line {
    let text: String = text
        .chars()
        .filter(|&c| c != '\r')
        .map(|c| if c == '\t' { ' ' } else { c })
        .collect();
    let mut pen = 0.0;
    let shaped = rusty_font::shape(font, &text, &[]);
    let mut out = Vec::with_capacity(shaped.len());
    for (i, g) in shaped.iter().enumerate() {
        out.push((g.glyph, Vec2::new(pen, 0.0) + g.offset));
        pen += g.advance;
        if shaped.get(i + 1).is_some_and(|n| n.cluster != g.cluster) {
            pen += tracking;
        }
    }
    (out, pen)
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
