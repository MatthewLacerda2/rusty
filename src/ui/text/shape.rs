//! src/ui/text/shape.rs — one run of text → positioned glyphs, with `harfrust` (#1328).
//!
//! A run is text in one face at one size; shaping picks its glyphs (ligatures,
//! contextual forms, combining marks) and positions them (GPOS kerning, mark
//! attachment). Positions come back in font units, which the caller scales to the
//! run's em size. A pure function of the font bytes and the string.

use glam::Vec2;

use super::font::{FontData, GlyphId};

/// One shaped glyph, font units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shaped {
    pub glyph: GlyphId,
    /// Byte offset in the run of the first character this glyph draws.
    pub cluster: u32,
    /// Pen advance after this glyph (kerning included).
    pub advance: f32,
    /// Offset of the glyph from the pen (mark attachment), y up.
    pub offset: Vec2,
}

/// Shape `run` in `font`, glyphs in visual order.
pub fn shape(font: &FontData, run: &str) -> Vec<Shaped> {
    if run.is_empty() {
        return Vec::new();
    }
    let face = font.face();
    let shaper = font.shaper.shaper(&face).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(run);
    buffer.guess_segment_properties();
    let out = shaper.shape(buffer, harfrust::ShapeOptions::new());
    out.glyph_infos()
        .iter()
        .zip(out.glyph_positions())
        .map(|(info, pos)| Shaped {
            glyph: GlyphId::new(info.glyph_id),
            cluster: info.cluster,
            advance: pos.x_advance as f32,
            offset: Vec2::new(pos.x_offset as f32, pos.y_offset as f32),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::text::font;

    fn advance_sum(s: &str) -> f32 {
        shape(&font::default_font(), s)
            .iter()
            .map(|g| g.advance)
            .sum()
    }

    #[test]
    fn kerning_tightens_classic_pairs() {
        let f = font::default_font();
        let lone = |c: &str| shape(&f, c)[0].advance;
        let pair = advance_sum("AV");
        assert!(pair < lone("A") + lone("V"), "AV kerns tighter");
    }

    #[test]
    fn the_same_bytes_and_string_shape_identically() {
        let f = font::default_font();
        let s = "Wavy fiords, ﬁne naïve café — AVATAR";
        assert_eq!(shape(&f, s), shape(&f, s));
        assert!(shape(&f, "").is_empty());
    }

    #[test]
    fn clusters_point_at_each_glyphs_first_byte() {
        let f = font::default_font();
        let g = shape(&f, "aé b");
        let clusters: Vec<u32> = g.iter().map(|s| s.cluster).collect();
        assert_eq!(clusters, vec![0, 1, 3, 4], "é is two bytes");
    }

    #[test]
    fn a_combining_mark_shares_its_base_cluster_and_does_not_advance() {
        let f = font::default_font();
        let g = shape(&f, "e\u{301}");
        assert!(g.iter().all(|s| s.cluster == 0), "{g:?}");
        if g.len() == 2 {
            assert_eq!(g[1].advance, 0.0, "a mark sits on its base");
        }
    }
}
