use std::collections::HashMap;

use glam::Vec2;

use super::sdf::{glyph_field, GlyphField};
use rusty_font::GlyphId;
use rusty_ui::text::font;

pub(crate) const WIDTH: u32 = 1024;
pub(crate) const START_HEIGHT: u32 = 256;
pub(crate) const MAX_HEIGHT: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AtlasGlyph {
    pub(crate) texels: [u32; 4],
    pub(crate) origin: Vec2,
}

pub(crate) struct GlyphAtlas {
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
    pub(crate) dirty: bool,
    glyphs: HashMap<GlyphId, Option<AtlasGlyph>>,
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
    pub(crate) fn glyph(
        &mut self,
        font: &rusty_font::FontData,
        glyph: GlyphId,
    ) -> Option<AtlasGlyph> {
        if let Some(g) = self.glyphs.get(&glyph) {
            return *g;
        }
        let placed = glyph_field(font, glyph).and_then(|f| self.insert(&f));
        self.glyphs.insert(glyph, placed);
        placed
    }

    fn insert(&mut self, field: &GlyphField) -> Option<AtlasGlyph> {
        let (w, h) = (field.width + 1, field.height + 1);
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
                    log::warn!(
                        "[Render] UI glyph atlas full ({WIDTH}×{MAX_HEIGHT}); glyphs skipped"
                    );
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

    pub(crate) fn uv(&self, g: &AtlasGlyph) -> (Vec2, Vec2) {
        let size = Vec2::new(WIDTH as f32, self.height as f32);
        let [x, y, w, h] = g.texels.map(|v| v as f32);
        (Vec2::new(x, y) / size, Vec2::new(x + w, y + h) / size)
    }
}

#[derive(Default)]
pub(crate) struct FontAtlases {
    pub(crate) atlases: HashMap<Option<String>, GlyphAtlas>,
    pub(crate) generation: u64,
}

impl FontAtlases {
    pub(crate) fn glyph(
        &mut self,
        path: &Option<String>,
        glyph: GlyphId,
    ) -> Option<(AtlasGlyph, (Vec2, Vec2))> {
        let atlas = self.atlases.entry(path.clone()).or_default();
        let height = atlas.height;
        let g = atlas.glyph(&font::load(path.as_deref()), glyph);
        self.generation += u64::from(atlas.height != height);
        let g = g?;
        Some((g, atlas.uv(&g)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_pack_once_and_the_atlas_grows_instead_of_moving_them() {
        let f = font::load(None);
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
