//! src/ui/text/layout.rs — a Text + its rect → placed glyphs (#419).
//!
//! [`layout_text`] is the whole CPU half of text: measure, wrap, apply overflow,
//! auto-size, align. Coordinates are **rect-local reference units, origin at the
//! rect's bottom-left, y up** (the `UiRect` frame); each glyph is placed by its
//! pen position on the baseline. The renderer maps these through the element's
//! corners and draws them from its SDF atlas; the sim reads [`preferred_size`] for
//! layout. Same inputs, same glyphs — headless or windowed.

use ab_glyph::GlyphId;
use glam::{Vec2, Vec4};

use super::lines::{block_height, break_lines, clip, pitch, Line, EPS};
use super::measure::Measurer;
use crate::components::TextComponent;

/// Binary-search steps auto-size takes between its bounds (a fixed count, so the
/// chosen size is a pure function of the inputs).
const AUTO_SIZE_STEPS: u32 = 12;

/// One glyph to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    pub glyph: GlyphId,
    /// Index into [`TextLayout::faces`].
    pub face: usize,
    /// Pen position on the baseline, rect-local reference units.
    pub origin: Vec2,
    /// Em size, reference units.
    pub size: f32,
    /// Display-space fill, straight alpha (rich-text colour folded in).
    pub color: Vec4,
    /// Synthesized bold (the renderer dilates the glyph).
    pub faux_bold: bool,
    /// Synthesized italic (the renderer shears the glyph).
    pub faux_italic: bool,
}

/// A laid-out text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    /// The font path of each face slot (`None`: the default font).
    pub faces: [Option<String>; 3],
    pub glyphs: Vec<PlacedGlyph>,
    /// The base size used (auto-size's pick, else `font_size`).
    pub font_size: f32,
    /// The drawn block's size: widest line × block height.
    pub size: Vec2,
    /// Lines drawn.
    pub lines: usize,
    /// Whether overflow cut anything.
    pub truncated: bool,
}

/// Lay `text` out in a `rect`-sized rect (reference units).
pub fn layout_text(text: &TextComponent, rect: Vec2) -> TextLayout {
    let size = if text.auto_size {
        auto_size(text, rect)
    } else {
        text.font_size
    };
    let measurer = Measurer::new(text, size);
    let mut lines = wrap(text, &measurer, rect.x);
    let truncated = clip(
        &mut lines,
        text.overflow,
        (rect.x, rect.y),
        text.line_spacing,
        &measurer,
    );
    let mut out = place(text, &lines, rect);
    out.font_size = size;
    out.truncated = truncated;
    out
}

/// The size `text` wants: `x` its widest line unwrapped, `y` its block height
/// when wrapped at `width` (unwrapped when `wrap` is off) — Unity's
/// `preferredWidth` / `preferredHeight`, at `font_size`.
pub fn preferred_size(text: &TextComponent, width: f32) -> Vec2 {
    let measurer = Measurer::new(text, text.font_size);
    let empty = measurer.base_metrics();
    let items = measurer.items();
    let unwrapped = break_lines(&items, None, empty);
    let w = unwrapped.iter().map(Line::width).fold(0.0, f32::max);
    let lines = if text.wrap {
        break_lines(&items, Some(width), empty)
    } else {
        unwrapped
    };
    Vec2::new(w, block_height(&lines, text.line_spacing))
}

fn wrap(text: &TextComponent, measurer: &Measurer, width: f32) -> Vec<Line> {
    let max = text.wrap.then_some(width);
    break_lines(&measurer.items(), max, measurer.base_metrics())
}

/// Whether `text` at `size` fits the rect without overflow.
fn fits(text: &TextComponent, rect: Vec2, size: f32) -> bool {
    let measurer = Measurer::new(text, size);
    let lines = wrap(text, &measurer, rect.x);
    block_height(&lines, text.line_spacing) <= rect.y + EPS
        && lines.iter().all(|l| l.width() <= rect.x + EPS)
}

/// The largest size in `[auto_size_min, auto_size_max]` that fits (the minimum
/// when none does).
fn auto_size(text: &TextComponent, rect: Vec2) -> f32 {
    let (mut lo, mut hi) = (
        text.auto_size_min,
        text.auto_size_max.max(text.auto_size_min),
    );
    if fits(text, rect, hi) {
        return hi;
    }
    for _ in 0..AUTO_SIZE_STEPS {
        let mid = 0.5 * (lo + hi);
        if fits(text, rect, mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Place every line's glyphs by the alignment.
fn place(text: &TextComponent, lines: &[Line], rect: Vec2) -> TextLayout {
    let align = text.alignment.fractions();
    let spacing = text.line_spacing;
    let height = block_height(lines, spacing);
    let top = (rect.y - height) * align.y + height;
    let mut glyphs = Vec::new();
    let mut widest: f32 = 0.0;
    let mut baseline = top - lines.first().map_or(0.0, |l| l.metrics.ascent);
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            baseline -= pitch(&lines[i - 1], line, spacing);
        }
        let width = line.width();
        widest = widest.max(width);
        let x0 = (rect.x - width) * align.x;
        for (x, it) in line.pens().filter(|(_, it)| !it.is_space()) {
            glyphs.push(PlacedGlyph {
                glyph: it.glyph,
                face: it.face,
                origin: Vec2::new(x0 + x, baseline),
                size: it.size,
                color: it.color,
                faux_bold: it.faux_bold,
                faux_italic: it.faux_italic,
            });
        }
    }
    TextLayout {
        faces: [
            text.font.clone(),
            text.font_bold.clone(),
            text.font_italic.clone(),
        ],
        glyphs,
        size: Vec2::new(widest, height),
        lines: lines.len(),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod layout_tests;
