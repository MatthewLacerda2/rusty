//! src/ui/text/font.rs — font assets: loading, the bundled default, metrics (#419, #1328).
//!
//! A font is a `.ttf` / `.otf` file referenced by path, like a texture; `None` is
//! the bundled default (Instrument Sans, SIL OFL 1.1 — the license sits beside it
//! in `engine/fonts/`). Parsed fonts are cached for the process by path, so the
//! sim's layout and the renderer's atlas read the very same glyph ids. A path that
//! does not load falls back to the default (with one warning), so a text never
//! vanishes over a typo.
//!
//! Faces are read with `skrifa`; shaping (`shape`, with `harfrust`) and outlines
//! (`outline`) are pure functions of the font bytes, so layout is deterministic.

use crate::core::collections::Map;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

use super::shape::Shaped;

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

pub use skrifa::GlyphId;

/// The bundled default font's bytes.
const DEFAULT_FONT: &[u8] = include_bytes!("../../../engine/fonts/InstrumentSans-Regular.ttf");

/// One loaded font: its bytes, the shaper's per-font tables and plans, and its metrics.
pub struct FontData {
    bytes: Arc<[u8]>,
    pub(super) shaper: harfrust::ShaperData,
    /// Shape plans by (direction, script), built on first use: a plan compiles the
    /// font's lookups, so building one per run would dominate a HUD's layout.
    pub(super) plans: Mutex<Vec<PlanEntry>>,
    /// Shaped runs by (text, unligated ranges): `shape` memoises here.
    pub(super) runs: Mutex<Map<RunKey, Arc<[Shaped]>>>,
    units_per_em: f32,
    /// Ascent, descent and line gap in font units, all positive.
    vertical: [f32; 3],
}

/// A cached shape plan and the segment properties it was built for.
pub(super) type PlanEntry = (
    harfrust::Direction,
    harfrust::Script,
    Arc<harfrust::ShapePlan>,
);

/// A shaped run's memo key: its text and the byte ranges kept unligated.
pub(super) type RunKey = (String, Vec<Range<usize>>);

/// A shared handle to a loaded font.
pub type FontHandle = Arc<FontData>;

impl FontData {
    fn parse(bytes: Arc<[u8]>) -> Option<Self> {
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
    pub(super) fn face(&self) -> FontRef<'_> {
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

type Cache = Mutex<Map<Option<String>, FontHandle>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Map::default()))
}

/// The bundled default font.
#[cfg(test)]
pub fn default_font() -> FontHandle {
    load(None)
}

/// The font at `path` (`None`: the bundled default), loading it on first use. A
/// path that fails to load resolves to the default font.
pub fn load(path: Option<&str>) -> FontHandle {
    let key = path.map(str::to_string);
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(f) = cache.get(&key) {
        return Arc::clone(f);
    }
    let parsed = path.and_then(|p| FontData::parse(std::fs::read(p).ok()?.into()));
    let font = match parsed {
        Some(f) => Arc::new(f),
        None => {
            if let Some(p) = path {
                log::warn!("[UI] font '{p}' did not load; using the default font");
            }
            Arc::clone(cache.entry(None).or_insert_with(|| Arc::new(bundled())))
        }
    };
    cache.insert(key, Arc::clone(&font));
    font
}

/// The bundled font, parsed. It is compiled in and known-good.
fn bundled() -> FontData {
    match FontData::parse(DEFAULT_FONT.into()) {
        Some(f) => f,
        None => unreachable!("the bundled default font parses"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_font_loads_and_a_bad_path_falls_back_to_it() {
        let d = default_font();
        assert!(d.glyph('A').is_some());
        let bad = load(Some("no/such/font.ttf"));
        assert!(
            Arc::ptr_eq(&d, &bad),
            "a missing font resolves to the default"
        );
    }

    #[test]
    fn bytes_that_are_not_a_font_do_not_parse() {
        assert!(FontData::parse(Arc::from(&b"not a font"[..])).is_none());
        let cut = &DEFAULT_FONT[..DEFAULT_FONT.len() / 3];
        // A truncated face either refuses to parse or still answers without panicking.
        if let Some(f) = FontData::parse(Arc::from(cut)) {
            let _ = f.glyph('A');
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
