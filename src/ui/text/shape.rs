//! src/ui/text/shape.rs — one run of text → positioned glyphs, with `harfrust` (#1328).
//!
//! A run is text in one face at one size; shaping picks its glyphs (ligatures,
//! contextual forms, combining marks) and positions them (GPOS kerning, mark
//! attachment). Positions come back in font units, which the caller scales to the
//! run's em size. A pure function of the font bytes and the string.

use std::ops::Range;
use std::sync::Arc;

use glam::Vec2;
use skrifa::instance::{LocationRef, Size};
use skrifa::MetadataProvider;

use super::font::{FontData, GlyphId};

/// One shaped glyph, font units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shaped {
    pub glyph: GlyphId,
    /// Byte offset in the run of the first character this glyph draws.
    pub cluster: u32,
    /// Pen advance after this glyph (kerning included).
    pub advance: f32,
    /// The advance with no kerning against the next glyph (`hmtx`; 0 for a
    /// glyph shaping zeroed, a mark).
    pub bare: f32,
    /// Offset of the glyph from the pen (mark attachment), y up.
    pub offset: Vec2,
}

/// Shaped runs kept per font before the memo starts over (a bound, not a tuning:
/// a HUD reshapes a few hundred strings, a counter adds one per new value).
const MEMO_RUNS: usize = 4096;

/// Shape `run` in `font`, glyphs in visual order. No ligature forms over a
/// character in `unligated` (byte ranges), so one never spans a style change;
/// kerning still does. Memoised per font: shaping is size-free, so auto-size's
/// probes and every frame after the first reuse one result.
pub fn shape(font: &FontData, run: &str, unligated: &[Range<usize>]) -> Arc<[Shaped]> {
    let key = (run.to_string(), unligated.to_vec());
    let mut memo = font.runs.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = memo.get(&key) {
        return Arc::clone(hit);
    }
    let shaped: Arc<[Shaped]> = shape_uncached(font, run, unligated).into();
    if memo.len() >= MEMO_RUNS {
        memo.clear();
    }
    memo.insert(key, Arc::clone(&shaped));
    shaped
}

fn shape_uncached(font: &FontData, run: &str, unligated: &[Range<usize>]) -> Vec<Shaped> {
    if run.is_empty() {
        return Vec::new();
    }
    let face = font.face();
    let shaper = font.shaper.shaper(&face).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(run);
    buffer.guess_segment_properties();
    let plan = plan(font, &shaper, buffer.direction(), buffer.script());
    let features: Vec<harfrust::Feature> = unligated
        .iter()
        .flat_map(|r| [LIGA, CLIG].map(|t| harfrust::Feature::new(t, 0, r.clone())))
        .collect();
    let options = harfrust::ShapeOptions::new()
        .plan(Some(&plan))
        .features(&features);
    let out = shaper.shape(buffer, options);
    let hmtx = face.glyph_metrics(Size::unscaled(), LocationRef::default());
    out.glyph_infos()
        .iter()
        .zip(out.glyph_positions())
        .map(|(info, pos)| Shaped {
            glyph: GlyphId::new(info.glyph_id),
            cluster: info.cluster,
            advance: pos.x_advance as f32,
            bare: match pos.x_advance {
                0 => 0.0,
                _ => hmtx
                    .advance_width(GlyphId::new(info.glyph_id))
                    .unwrap_or(0.0),
            },
            offset: Vec2::new(pos.x_offset as f32, pos.y_offset as f32),
        })
        .collect()
}

const LIGA: harfrust::Tag = harfrust::Tag::new(b"liga");
const CLIG: harfrust::Tag = harfrust::Tag::new(b"clig");

/// `font`'s plan for `direction` and `script`, built once. It is built with
/// ranged `liga`/`clig` features so their mask bits exist; the ranges each call
/// passes are what turn them off.
fn plan(
    font: &FontData,
    shaper: &harfrust::Shaper,
    direction: harfrust::Direction,
    script: harfrust::Script,
) -> Arc<harfrust::ShapePlan> {
    let mut plans = font.plans.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, _, p)) = plans
        .iter()
        .find(|(d, s, _)| *d == direction && *s == script)
    {
        return Arc::clone(p);
    }
    let ranged = [LIGA, CLIG].map(|t| harfrust::Feature::new(t, 0, 0..1));
    let p = Arc::new(harfrust::ShapePlan::new(
        shaper,
        direction,
        Some(script),
        None,
        &ranged,
    ));
    plans.push((direction, script, Arc::clone(&p)));
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::text::font;

    fn advance_sum(s: &str) -> f32 {
        shape(&font::default_font(), s, &[])
            .iter()
            .map(|g| g.advance)
            .sum()
    }

    #[test]
    fn kerning_tightens_classic_pairs() {
        let f = font::default_font();
        let lone = |c: &str| shape(&f, c, &[])[0].advance;
        let pair = advance_sum("AV");
        assert!(pair < lone("A") + lone("V"), "AV kerns tighter");
    }

    #[test]
    fn the_same_bytes_and_string_shape_identically() {
        let f = font::default_font();
        let s = "Wavy fiords, ﬁne naïve café — AVATAR";
        assert_eq!(shape(&f, s, &[]), shape(&f, s, &[]));
        assert!(shape(&f, "", &[]).is_empty());
    }

    #[test]
    fn clusters_point_at_each_glyphs_first_byte() {
        let f = font::default_font();
        let g = shape(&f, "aé b", &[]);
        let clusters: Vec<u32> = g.iter().map(|s| s.cluster).collect();
        assert_eq!(clusters, vec![0, 1, 3, 4], "é is two bytes");
    }

    #[test]
    fn a_combining_mark_shares_its_base_cluster_and_does_not_advance() {
        let f = font::default_font();
        let g = shape(&f, "q\u{301}", &[]);
        assert_eq!(g.len(), 2, "no precomposed q-acute: the mark stays");
        assert!(g.iter().all(|s| s.cluster == 0), "{g:?}");
        assert_eq!(
            (g[1].advance, g[1].bare),
            (0.0, 0.0),
            "a mark sits on its base"
        );
    }

    #[test]
    fn an_unligated_range_splits_a_ligature_and_keeps_kerning() {
        let f = font::default_font();
        assert_eq!(shape(&f, "fi", &[]).len(), 1);
        assert_eq!(shape(&f, "fi", &[0..2]).len(), 2);
        let sum = |g: &[Shaped]| g.iter().map(|s| s.advance).sum::<f32>();
        assert_eq!(sum(&shape(&f, "AV", &[0..2])), advance_sum("AV"));
        assert!(shape(&f, "AV", &[])[0].bare > shape(&f, "AV", &[])[0].advance);
    }
}
