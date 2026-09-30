//! src/ui/text/measure.rs — styled characters → measured items (#419).
//!
//! Resolves each character's face (a real bold / italic font when the text names
//! one, else the regular face with a synthesized style), its glyph, advance,
//! kerning against its predecessor and its line metrics — all in reference units
//! at the character's own size. Pure CPU; the same numbers headless and windowed.

use ab_glyph::{Font, GlyphId, ScaleFont};
use glam::Vec4;

use super::font::{self, FontHandle};
use super::rich::{self, Style};
use crate::components::TextComponent;

/// Extra advance a synthesized bold adds, in ems (the dilated stroke is wider).
pub const FAUX_BOLD_ADVANCE: f32 = 0.03;

/// The face slots: the regular, bold and italic fonts a text names.
pub const REGULAR: usize = 0;
pub const BOLD: usize = 1;
pub const ITALIC: usize = 2;

/// One measured character.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub ch: char,
    pub glyph: GlyphId,
    /// The face slot ([`REGULAR`], [`BOLD`] or [`ITALIC`]).
    pub face: usize,
    /// Em size, reference units.
    pub size: f32,
    pub color: Vec4,
    pub faux_bold: bool,
    pub faux_italic: bool,
    /// Pen advance after this character (letter spacing and faux bold included).
    pub advance: f32,
    /// Kerning against the previous character on the same line (same face and size).
    pub kern: f32,
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

    /// Every character of the text (`\n` included), measured.
    pub fn items(&self) -> Vec<Item> {
        let styled = rich::parse(&self.text.text, self.text.rich_text, self.text.color);
        let mut out: Vec<Item> = Vec::with_capacity(styled.len());
        for (ch, style) in styled {
            let prev = out.last().copied();
            out.push(self.item(ch, style, prev));
        }
        out
    }

    /// An ellipsis styled like `like`: `…`, or `...` when the face lacks it.
    pub fn ellipsis(&self, like: &Item) -> Vec<Item> {
        let style = Style {
            color: like.color,
            bold: like.faux_bold || like.face == BOLD,
            italic: like.faux_italic || like.face == ITALIC,
            size: Some(like.size * self.authored() / self.size),
        };
        let chars: &[char] = if self.faces[like.face].glyph('…').is_some() {
            &['…']
        } else {
            &['.', '.', '.']
        };
        let mut out: Vec<Item> = Vec::new();
        for &c in chars {
            let prev = out.last().copied();
            out.push(self.item(c, style, prev));
        }
        out
    }

    /// The authored `font_size` `<size>` tags are relative to (kept away from 0).
    fn authored(&self) -> f32 {
        self.text.font_size.max(1e-3)
    }

    fn item(&self, ch: char, style: Style, prev: Option<Item>) -> Item {
        let t = self.text;
        let face = match (style.bold && t.font_bold.is_some(), style.italic) {
            (true, _) => BOLD,
            (false, true) if t.font_italic.is_some() => ITALIC,
            _ => REGULAR,
        };
        let size = style
            .size
            .map_or(self.size, |s| s * self.size / self.authored());
        let font = &self.faces[face];
        let glyph = font.font().glyph_id(if ch == '\t' { ' ' } else { ch });
        let scaled = font.font().as_scaled(font.em_scale(size));
        let faux_bold = style.bold && face != BOLD;
        let bold_extra = if faux_bold { FAUX_BOLD_ADVANCE } else { 0.0 };
        let advance = scaled.h_advance(glyph) + (t.letter_spacing + bold_extra) * size;
        let kern = match prev {
            Some(p) if p.face == face && p.size == size => {
                font.kern_units(p.glyph, glyph) * size / font.units_per_em()
            }
            _ => 0.0,
        };
        Item {
            ch,
            glyph,
            face,
            size,
            color: style.color,
            faux_bold,
            faux_italic: style.italic && face != ITALIC,
            advance,
            kern,
            metrics: metrics(font, size),
        }
    }
}

fn metrics(font: &FontHandle, size: f32) -> LineMetrics {
    let scaled = font.font().as_scaled(font.em_scale(size));
    LineMetrics {
        ascent: scaled.ascent(),
        descent: -scaled.descent(),
        gap: scaled.line_gap(),
    }
}
