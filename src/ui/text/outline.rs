//! src/ui/text/outline.rs — a glyph's outline, flattened to line segments (#1328).
//!
//! `skrifa` draws the glyph unhinted at a pixel size; its quadratic and cubic
//! curves are cut into a fixed number of chords from the control polygon's
//! length, so the segment list is a pure function of the font bytes and the size.
//! The renderer fills these segments (non-zero winding) for the SDF atlas.

use glam::Vec2;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;

use super::font::{FontData, GlyphId};

/// Longest chord a curve is cut into, pixels.
const CHORD: f32 = 2.0;
/// Most chords one curve is cut into.
const MAX_CHORDS: u32 = 64;

/// One edge of a glyph's outline, pixels from the pen, y up.
pub type Segment = [Vec2; 2];

impl FontData {
    /// `glyph`'s outline at `ppem` pixels per em, flattened; `None` for an empty
    /// glyph (a space) or one that fails to draw.
    pub fn outline(&self, glyph: GlyphId, ppem: f32) -> Option<Vec<Segment>> {
        let face = self.face();
        let outline = face.outline_glyphs().get(glyph)?;
        let mut pen = Flatten::default();
        let settings = DrawSettings::unhinted(Size::new(ppem), LocationRef::default());
        outline.draw(settings, &mut pen).ok()?;
        pen.close();
        (!pen.segments.is_empty()).then_some(pen.segments)
    }
}

/// An [`OutlinePen`] that keeps line segments.
#[derive(Default)]
struct Flatten {
    segments: Vec<Segment>,
    start: Vec2,
    at: Vec2,
}

impl Flatten {
    fn line(&mut self, to: Vec2) {
        if to != self.at {
            self.segments.push([self.at, to]);
        }
        self.at = to;
    }

    /// Chords for a curve whose control polygon is `points` long.
    fn chords(points: &[Vec2]) -> u32 {
        let len: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        ((len / CHORD).ceil() as u32).clamp(1, MAX_CHORDS)
    }
}

impl OutlinePen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.start = Vec2::new(x, y);
        self.at = self.start;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.line(Vec2::new(x, y));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (p0, c, p1) = (self.at, Vec2::new(cx, cy), Vec2::new(x, y));
        let n = Self::chords(&[p0, c, p1]);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            self.line(p0 * (u * u) + c * (2.0 * u * t) + p1 * (t * t));
        }
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let p0 = self.at;
        let (c0, c1, p1) = (Vec2::new(cx0, cy0), Vec2::new(cx1, cy1), Vec2::new(x, y));
        let n = Self::chords(&[p0, c0, c1, p1]);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let p = p0 * (u * u * u) + c0 * (3.0 * u * u * t) + c1 * (3.0 * u * t * t);
            self.line(p + p1 * (t * t * t));
        }
    }

    fn close(&mut self) {
        let start = self.start;
        self.line(start);
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::text::font;

    #[test]
    fn an_outline_is_closed_and_scales_with_its_size() {
        let f = font::default_font();
        let o = f.outline(f.glyph('O').expect("O"), 100.0).expect("O draws");
        let net = o.iter().fold(glam::Vec2::ZERO, |s, [a, b]| s + (*b - *a));
        assert!(net.length() < 1e-3, "every contour closes: {net}");
        let top = |o: &[[glam::Vec2; 2]]| o.iter().map(|s| s[0].y).fold(0.0, f32::max);
        let big = f.outline(f.glyph('O').expect("O"), 200.0).expect("O draws");
        assert!((top(&big) - 2.0 * top(&o)).abs() < 1.0, "twice the size");
        assert!(f.outline(f.glyph(' ').expect("space"), 100.0).is_none());
    }
}
