//! src/render/ui/text/atlas.rs — the per-font dynamic SDF atlas (CPU side, #419).
//!
//! One single-channel atlas per font asset. A glyph's field is generated the first
//! time a text draws it and shelf-packed into the atlas, which is marked dirty so
//! the pass re-uploads it; the atlas starts [`WIDTH`]×[`START_HEIGHT`] and doubles
//! its height as it fills, up to [`MAX_HEIGHT`] (past that a new glyph is skipped,
//! with one warning). Placement is in texels, so growing never moves a glyph.

use std::collections::HashMap;

use ab_glyph::GlyphId;
use glam::Vec2;

use super::sdf::{glyph_field, GlyphField};
use crate::ui::text::font;

pub(crate) const WIDTH: u32 = 1024;
pub(crate) const START_HEIGHT: u32 = 256;
pub(crate) const MAX_HEIGHT: u32 = 4096;

/// Where one glyph sits: its texel rect and its quad relative to the pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AtlasGlyph {
    /// Texel rect `(x, y, w, h)`, top-left origin.
    pub(crate) texels: [u32; 4],
    /// The quad's bottom-left relative to the pen, base pixels, y up.
    pub(crate) origin: Vec2,
}

/// One font's atlas.
pub(crate) struct GlyphAtlas {
    pub(crate) height: u32,
    /// `WIDTH × height` coverage, row-major, top row first.
    pub(crate) pixels: Vec<u8>,
    /// Changed since the last upload.
    pub(crate) dirty: bool,
    glyphs: HashMap<u16, Option<AtlasGlyph>>,
    /// Shelf packer: the cursor and the current shelf's height.
    cursor: (u32, u32),
    shelf: u32,
    warned: bool,
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self {
            height: START_HEIGHT,
            pixels: vec![0; (WIDTH * START_HEIGHT) as usize],
            dirty: true,
            glyphs: HashMap::new(),
            cursor: (0, 0),
            shelf: 0,
            warned: false,
        }
    }
}

impl GlyphAtlas {
    /// `glyph`'s place, generating and packing it on first use. `None` for an empty
    /// glyph or a full atlas.
    pub(crate) fn glyph(&mut self, font: &font::FontData, glyph: GlyphId) -> Option<AtlasGlyph> {
        if let Some(g) = self.glyphs.get(&glyph.0) {
            return *g;
        }
        let placed = glyph_field(font, glyph).and_then(|f| self.insert(&f));
        self.glyphs.insert(glyph.0, placed);
        placed
    }

    /// Pack `field`, growing the atlas when needed.
    fn insert(&mut self, field: &GlyphField) -> Option<AtlasGlyph> {
        let (w, h) = (field.width + 1, field.height + 1); // a gutter against bleeding
        if w > WIDTH {
            return None;
        }
        if self.cursor.0 + w > WIDTH {
            self.cursor = (0, self.cursor.1 + self.shelf);
            self.shelf = 0;
        }
        while self.cursor.1 + h > self.height {
            if self.height >= MAX_HEIGHT {
                if !self.warned {
                    log::warn!("UI glyph atlas full ({WIDTH}×{MAX_HEIGHT}); glyphs skipped");
                    self.warned = true;
                }
                return None;
            }
            self.height *= 2;
            self.pixels.resize((WIDTH * self.height) as usize, 0);
        }
        let (x, y) = self.cursor;
        for row in 0..field.height {
            let src = (row * field.width) as usize;
            let dst = ((y + row) * WIDTH + x) as usize;
            self.pixels[dst..dst + field.width as usize]
                .copy_from_slice(&field.pixels[src..src + field.width as usize]);
        }
        self.cursor.0 += w;
        self.shelf = self.shelf.max(h);
        self.dirty = true;
        Some(AtlasGlyph {
            texels: [x, y, field.width, field.height],
            origin: field.origin,
        })
    }

    /// A glyph's texture coordinates `(min, max)` in the atlas's current size (v down).
    pub(crate) fn uv(&self, g: &AtlasGlyph) -> (Vec2, Vec2) {
        let size = Vec2::new(WIDTH as f32, self.height as f32);
        let [x, y, w, h] = g.texels.map(|v| v as f32);
        (Vec2::new(x, y) / size, Vec2::new(x + w, y + h) / size)
    }
}

/// Every font's atlas, keyed by font path (`None`: the default font).
#[derive(Default)]
pub(crate) struct FontAtlases {
    pub(crate) atlases: HashMap<Option<String>, GlyphAtlas>,
}

impl FontAtlases {
    /// `glyph` of the font at `path`: its place and its uv rect.
    pub(crate) fn glyph(
        &mut self,
        path: &Option<String>,
        glyph: GlyphId,
    ) -> Option<(AtlasGlyph, (Vec2, Vec2))> {
        let atlas = self.atlases.entry(path.clone()).or_default();
        let g = atlas.glyph(&font::load(path.as_deref()), glyph)?;
        Some((g, atlas.uv(&g)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_pack_once_and_the_atlas_grows_instead_of_moving_them() {
        let f = font::default_font();
        let mut atlas = GlyphAtlas::default();
        let a = atlas.glyph(&f, f.glyph('A').expect("A")).expect("packed");
        atlas.dirty = false;
        assert_eq!(atlas.glyph(&f, f.glyph('A').expect("A")), Some(a));
        assert!(!atlas.dirty, "a cached glyph re-packs nothing");
        let chars: Vec<char> = ('!'..='~').chain('À'..='ÿ').collect();
        for c in chars {
            if let Some(g) = f.glyph(c) {
                atlas.glyph(&f, g);
            }
        }
        assert!(atlas.height > START_HEIGHT, "grew: {}", atlas.height);
        assert_eq!(
            atlas.glyph(&f, f.glyph('A').expect("A")),
            Some(a),
            "never moved"
        );
        assert!(atlas.glyph(&f, f.glyph(' ').expect("space")).is_none());
    }
}
