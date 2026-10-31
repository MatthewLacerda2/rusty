//! src/ui/text/measure.rs — styled characters → measured glyphs (#419, #1328).
//!
//! Resolves each character's face (a real bold / italic font when the text names
//! one, else the regular face with a synthesized style), splits the text into runs
//! of one face and size, and shapes each run: an item is one shaped glyph, with
//! its advance (kerning included), its offset (mark attachment) and its line
//! metrics — all in reference units at its own size. Pure CPU; the same numbers
//! headless and windowed.

use glam::{Vec2, Vec4};

use super::font::{self, FontHandle, GlyphId};
use super::rich::{self, Style};
use super::shape;
use crate::components::TextComponent;

/// Extra advance a synthesized bold adds, in ems (the dilated stroke is wider).
const FAUX_BOLD_ADVANCE: f32 = 0.03;

/// The face slots: the regular, bold and italic fonts a text names.
const REGULAR: usize = 0;
const BOLD: usize = 1;
const ITALIC: usize = 2;

/// One measured glyph.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    /// The first character the glyph draws (a ligature draws several).
    pub ch: char,
    pub glyph: GlyphId,
    /// The face slot ([`REGULAR`], [`BOLD`] or [`ITALIC`]).
    pub face: usize,
    /// Em size, reference units.
    pub size: f32,
    pub color: Vec4,
    pub faux_bold: bool,
    pub faux_italic: bool,
    /// Pen advance after this glyph (kerning, letter spacing and faux bold included).
    pub advance: f32,
    /// Where the glyph sits relative to the pen (a mark's attachment), y up.
    pub offset: Vec2,
    /// Part of the previous glyph's cluster (a mark on its base): never broken from it.
    pub joins: bool,
    pub metrics: LineMetrics,
}

impl Item {
    pub fn is_space(&self) -> bool {
        self.ch.is_whitespace()
    }
}

/// Vertical metrics, reference units, all positive.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub gap: f32,
}

impl LineMetrics {
    pub fn max(self, o: LineMetrics) -> LineMetrics {
        LineMetrics {
            ascent: self.ascent.max(o.ascent),
            descent: self.descent.max(o.descent),
            gap: self.gap.max(o.gap),
        }
    }
}

/// Measures one text at one base size.
pub struct Measurer<'a> {
    text: &'a TextComponent,
    faces: [FontHandle; 3],
    /// The base em size (auto-sized or `font_size`).
    size: f32,
}

impl<'a> Measurer<'a> {
    pub fn new(text: &'a TextComponent, size: f32) -> Self {
        let face = |p: &Option<String>| font::load(p.as_deref().or(text.font.as_deref()));
        Self {
            text,
            faces: [
                font::load(text.font.as_deref()),
                face(&text.font_bold),
                face(&text.font_italic),
            ],
            size,
        }
    }

    /// The base size's metrics — what an empty line is as tall as.
    pub fn base_metrics(&self) -> LineMetrics {
        metrics(&self.faces[REGULAR], self.size)
    }

    /// Every glyph of the text (`\n` included, as its own item), measured.
    pub fn items(&self) -> Vec<Item> {
        self.measure(&rich::parse(
            &self.text.text,
            self.text.rich_text,
            self.text.color,
        ))
    }

    /// An ellipsis styled like `like`: `…`, or `...` when the face lacks it.
    pub fn ellipsis(&self, like: &Item) -> Vec<Item> {
        let style = Style {
            color: like.color,
            bold: like.faux_bold || like.face == BOLD,
            italic: like.faux_italic || like.face == ITALIC,
            size: Some(like.size * self.authored() / self.size),
        };
        let dots = if self.faces[like.face].glyph('…').is_some() {
            "…"
        } else {
            "..."
        };
        self.measure(&dots.chars().map(|c| (c, style)).collect::<Vec<_>>())
    }

    /// The authored `font_size` `<size>` tags are relative to (kept away from 0).
    fn authored(&self) -> f32 {
        self.text.font_size.max(1e-3)
    }

    /// The face slot and em size `style` draws in.
    fn slot(&self, style: &Style) -> (usize, f32) {
        let t = self.text;
        let face = match (style.bold && t.font_bold.is_some(), style.italic) {
            (true, _) => BOLD,
            (false, true) if t.font_italic.is_some() => ITALIC,
            _ => REGULAR,
        };
        let size = style
            .size
            .map_or(self.size, |s| s * self.size / self.authored());
        (face, size)
    }

    /// Split `styled` into runs of one face and size (`\n` alone) and shape each.
    fn measure(&self, styled: &[(char, Style)]) -> Vec<Item> {
        let mut out = Vec::with_capacity(styled.len());
        let mut start = 0;
        while start < styled.len() {
            let key = self.slot(&styled[start].1);
            let newline = styled[start].0 == '\n';
            let len = if newline {
                1
            } else {
                styled[start..]
                    .iter()
                    .take_while(|(c, s)| *c != '\n' && self.slot(s) == key)
                    .count()
            };
            let run = &styled[start..start + len];
            if newline {
                out.push(self.newline(run[0], key));
            } else {
                self.run(run, key, &mut out);
            }
            start += len;
        }
        out
    }

    /// Shape one run (one face, one size, no `\n`) into `out`.
    fn run(&self, run: &[(char, Style)], (face, size): (usize, f32), out: &mut Vec<Item>) {
        let t = self.text;
        let font = &self.faces[face];
        // A tab shapes as a space: one byte each, so clusters still line up.
        let text: String = run
            .iter()
            .map(|&(c, _)| if c == '\t' { ' ' } else { c })
            .collect();
        let starts: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        let scale = size / font.units_per_em();
        let mut prev_cluster = None;
        for g in shape::shape(font, &text) {
            let at = starts.partition_point(|&b| b <= g.cluster as usize).max(1) - 1;
            let (ch, style) = run[at];
            let faux_bold = style.bold && face != BOLD;
            let bold_extra = if faux_bold { FAUX_BOLD_ADVANCE } else { 0.0 };
            out.push(Item {
                ch,
                glyph: g.glyph,
                face,
                size,
                color: style.color,
                faux_bold,
                faux_italic: style.italic && face != ITALIC,
                advance: g.advance * scale + (t.letter_spacing + bold_extra) * size,
                offset: g.offset * scale,
                joins: prev_cluster == Some(g.cluster),
                metrics: metrics(font, size),
            });
            prev_cluster = Some(g.cluster);
        }
    }

    /// A line break: no glyph and no advance, only the line metrics it ends with.
    fn newline(&self, (ch, style): (char, Style), (face, size): (usize, f32)) -> Item {
        Item {
            ch,
            glyph: GlyphId::NOTDEF,
            face,
            size,
            color: style.color,
            faux_bold: false,
            faux_italic: false,
            advance: 0.0,
            offset: Vec2::ZERO,
            joins: false,
            metrics: metrics(&self.faces[face], size),
        }
    }
}

fn metrics(font: &FontHandle, size: f32) -> LineMetrics {
    let [ascent, descent, gap] = font.vertical_metrics(size);
    LineMetrics {
        ascent,
        descent,
        gap,
    }
}
