//! crates/font/src/face.rs — one parsed font: its bytes, metrics and glyphs, and the
//! bundled default (#419, #1328).
//!
//! A font is parsed from bytes the caller read: this crate reads no files. The
//! default is Instrument Sans (SIL OFL 1.1 — the license sits beside it in
//! `engine/fonts/`), compiled in. Faces are read with `skrifa`; shaping
//! ([`crate::shape`], with `harfrust`) and outlines ([`FontData::outline`]) are pure
//! functions of the font bytes, so what they give is deterministic.

use rusty_core::collections::Map;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

use crate::shape::Shaped;

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

pub use skrifa::GlyphId;

/// The bundled default font's bytes.
const DEFAULT_FONT: &[u8] = include_bytes!("../../../engine/fonts/InstrumentSans-Regular.ttf");

/// One loaded font: its bytes, the shaper's per-font tables and plans, and its metrics.
pub struct FontData {
    bytes: Arc<[u8]>,
    pub(crate) shaper: harfrust::ShaperData,
    /// Shape plans by (direction, script), built on first use: a plan compiles the
    /// font's lookups, so building one per run would dominate a HUD's layout.
    pub(crate) plans: Mutex<Vec<PlanEntry>>,
    /// Shaped runs by (text, unligated ranges): `shape` memoises here.
    pub(crate) runs: Mutex<Map<RunKey, Arc<[Shaped]>>>,
    units_per_em: f32,
    /// Ascent, descent and line gap in font units, all positive.
    vertical: [f32; 3],
}

/// A cached shape plan and the segment properties it was built for.
pub(crate) type PlanEntry = (
    harfrust::Direction,
    harfrust::Script,
    Arc<harfrust::ShapePlan>,
);

/// A shaped run's memo key: its text and the byte ranges kept unligated.
pub(crate) type RunKey = (String, Vec<Range<usize>>);

/// A shared handle to a loaded font.
pub type FontHandle = Arc<FontData>;

impl FontData {
    /// Parse a `.ttf` / `.otf` file's bytes; `None` when they are no font this reads
    /// (truncated, another format, a zero em).
    pub fn parse(bytes: Arc<[u8]>) -> Option<Self> {
        let face = FontRef::new(&bytes).ok()?;
        let m = face.metrics(Size::unscaled(), LocationRef::default());
        if m.units_per_em == 0 {
            return None;
        }
        let shaper = harfrust::ShaperData::new(&face);
        Some(Self {
            units_per_em: f32::from(m.units_per_em),
            vertical: [m.ascent, -m.descent, m.leading.max(0.0)],
            shaper,
            plans: Mutex::default(),
            runs: Mutex::default(),
            bytes,
        })
    }

    /// The parsed face. The bytes parsed once in [`FontData::parse`], so this holds.
    pub(crate) fn face(&self) -> FontRef<'_> {
        match FontRef::new(&self.bytes) {
            Ok(f) => f,
            Err(_) => unreachable!("a FontData's bytes parsed when it loaded"),
        }
    }

    /// Font units per em.
    pub fn units_per_em(&self) -> f32 {
        self.units_per_em
    }

    /// Ascent, descent and line gap at em size `size`, all positive.
    pub fn vertical_metrics(&self, size: f32) -> [f32; 3] {
        self.vertical.map(|v| v * size / self.units_per_em)
    }

    /// The glyph for `c`, or `None` when the font lacks it.
    pub fn glyph(&self, c: char) -> Option<GlyphId> {
        self.face()
            .charmap()
            .map(c)
            .filter(|g| *g != GlyphId::NOTDEF)
    }
}

/// The engine's default font, `engine/fonts/InstrumentSans-Regular.ttf`, compiled
/// in, so a shipped game draws it with no file beside it. Parsed once per process.
pub fn default_font() -> FontHandle {
    static DEFAULT: OnceLock<FontHandle> = OnceLock::new();
    Arc::clone(
        DEFAULT.get_or_init(|| match FontData::parse(DEFAULT_FONT.into()) {
            Some(f) => Arc::new(f),
            None => unreachable!("the bundled default font parses"),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_font_loads_once() {
        let d = default_font();
        assert!(d.glyph('A').is_some());
        assert!(Arc::ptr_eq(&d, &default_font()));
    }

    #[test]
    fn bytes_that_are_not_a_font_do_not_parse() {
        assert!(FontData::parse(Arc::from(&b"not a font"[..])).is_none());
        assert!(FontData::parse(Arc::from(&b""[..])).is_none());
        let cut = &DEFAULT_FONT[..DEFAULT_FONT.len() / 3];
        // A truncated face either refuses to parse or still answers without panicking.
        if let Some(f) = FontData::parse(Arc::from(cut)) {
            let _ = f.glyph('A');
            if let Some(g) = f.glyph('A') {
                let _ = f.outline(g, 100.0);
            }
            let _ = crate::shape(&f, "AV", &[]);
        }
    }

    #[test]
    fn metrics_scale_with_the_em_size() {
        let d = default_font();
        let [a, desc, _] = d.vertical_metrics(10.0);
        let [a2, desc2, _] = d.vertical_metrics(20.0);
        assert!(a > 0.0 && desc > 0.0);
        assert!((a2 - 2.0 * a).abs() < 1e-4 && (desc2 - 2.0 * desc).abs() < 1e-4);
    }
}
