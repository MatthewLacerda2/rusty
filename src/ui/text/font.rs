//! src/ui/text/font.rs — font assets: loading, the bundled default, kerning (#419).
//!
//! A font is a `.ttf` / `.otf` file referenced by path, like a texture; `None` is
//! the bundled default (Instrument Sans, SIL OFL 1.1 — the license sits beside it
//! in `engine/fonts/`). Parsed fonts are cached for the process by path, so the
//! sim's layout and the renderer's atlas read the very same glyph ids. A path that
//! does not load falls back to the default (with one warning), so a text never
//! vanishes over a typo.
//!
//! Kerning is read from the font's GPOS `kern` feature (pair adjustment, both
//! formats) — what modern fonts ship — falling back to the legacy `kern` table.
//! Full shaping (ligatures, marks, RTL, CJK) is out of scope (`docs/ui.md`).

use crate::core::collections::Map;
use std::sync::{Arc, Mutex, OnceLock};

use ab_glyph::{Font, FontArc, GlyphId, PxScale};

/// The bundled default font's bytes.
const DEFAULT_FONT: &[u8] = include_bytes!("../../../engine/fonts/InstrumentSans-Regular.ttf");

/// One loaded font: the parsed face plus its kerning lookups.
pub struct FontData {
    font: FontArc,
    bytes: Arc<[u8]>,
    /// GPOS lookup indices the `kern` feature names (empty: use the `kern` table).
    kern_lookups: Vec<u16>,
    /// Pair kerning already resolved, in font units.
    kern_memo: Mutex<Map<(u16, u16), f32>>,
}

/// A shared handle to a loaded font.
pub type FontHandle = Arc<FontData>;

impl FontData {
    fn parse(bytes: Arc<[u8]>) -> Option<Self> {
        let font = FontArc::try_from_vec(bytes.to_vec()).ok()?;
        let kern_lookups = kern_lookups(&bytes);
        Some(Self {
            font,
            bytes,
            kern_lookups,
            kern_memo: Mutex::new(Map::default()),
        })
    }

    /// The parsed face (glyph ids, advances, outlines).
    pub fn font(&self) -> &FontArc {
        &self.font
    }

    /// The `PxScale` that makes one em `size` pixels (ab_glyph scales by
    /// `ascent - descent`, UI sizes are ems, as in Unity and CSS).
    pub fn em_scale(&self, size: f32) -> PxScale {
        let upem = self.font.units_per_em().unwrap_or(1000.0);
        PxScale::from(size * self.font.height_unscaled() / upem)
    }

    /// Font units per em.
    pub fn units_per_em(&self) -> f32 {
        self.font.units_per_em().unwrap_or(1000.0)
    }

    /// The glyph for `c`, or `None` when the font lacks it.
    pub fn glyph(&self, c: char) -> Option<GlyphId> {
        let id = self.font.glyph_id(c);
        (id.0 != 0).then_some(id)
    }

    /// The horizontal kerning between `a` and `b`, in font units.
    pub fn kern_units(&self, a: GlyphId, b: GlyphId) -> f32 {
        let key = (a.0, b.0);
        let mut memo = self.kern_memo.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(k) = memo.get(&key) {
            return *k;
        }
        let k = if self.kern_lookups.is_empty() {
            self.font.kern_unscaled(a, b)
        } else {
            gpos_kern(&self.bytes, &self.kern_lookups, a, b).unwrap_or(0.0)
        };
        memo.insert(key, k);
        k
    }
}

type Cache = Mutex<Map<Option<String>, FontHandle>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Map::default()))
}

/// The bundled default font.
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
                log::warn!("font '{p}' did not load; using the default font");
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

/// The GPOS lookups the `kern` feature uses, in order, deduplicated.
fn kern_lookups(bytes: &[u8]) -> Vec<u16> {
    let Ok(face) = ttf_parser::Face::parse(bytes, 0) else {
        return Vec::new();
    };
    let Some(gpos) = face.tables().gpos else {
        return Vec::new();
    };
    let tag = ttf_parser::Tag::from_bytes(b"kern");
    let mut out: Vec<u16> = gpos
        .features
        .into_iter()
        .filter(|f| f.tag == tag)
        .flat_map(|f| f.lookup_indices)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The first pair adjustment among `lookups` that covers `(a, b)`, as the first
/// glyph's x-advance change in font units.
fn gpos_kern(bytes: &[u8], lookups: &[u16], a: GlyphId, b: GlyphId) -> Option<f32> {
    use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let gpos = face.tables().gpos?;
    let (a, b) = (ttf_parser::GlyphId(a.0), ttf_parser::GlyphId(b.0));
    for &index in lookups {
        let Some(lookup) = gpos.lookups.get(index) else {
            continue;
        };
        for sub in lookup.subtables.into_iter::<PositioningSubtable>() {
            let PositioningSubtable::Pair(pair) = sub else {
                continue;
            };
            let hit = match pair {
                PairAdjustment::Format1 { coverage, sets } => coverage
                    .get(a)
                    .and_then(|i| sets.get(i))
                    .and_then(|set| set.get(b)),
                PairAdjustment::Format2 {
                    coverage,
                    classes,
                    matrix,
                } => coverage
                    .get(a)
                    .and_then(|_| matrix.get((classes.0.get(a), classes.1.get(b)))),
            };
            if let Some((first, _)) = hit {
                return Some(f32::from(first.x_advance));
            }
        }
    }
    None
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
    fn gpos_kerning_tightens_classic_pairs() {
        let d = default_font();
        let (a, v) = (d.glyph('A'), d.glyph('V'));
        let (Some(a), Some(v)) = (a, v) else {
            panic!("the default font has A and V")
        };
        assert!(d.kern_units(a, v) < 0.0, "AV kerns tighter");
        assert_eq!(d.kern_units(a, v), d.kern_units(a, v), "memoized");
    }
}
