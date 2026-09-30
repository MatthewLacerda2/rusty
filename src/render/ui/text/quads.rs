//! src/render/ui/text/quads.rs — a laid-out Text → SDF glyph quads (#419).
//!
//! Lays the text out with the sim's own `ui::text::layout_text`, then turns each
//! glyph into a quad over its atlas field, in **rect-local reference units**
//! (origin at the rect's bottom-left, y up) — the mesh builder maps them through
//! the element's corners. The drop shadow is a second copy of every glyph, offset
//! and emitted first, so it never covers a neighbouring glyph. Synthesized italic
//! shears the quad; synthesized bold, the outline and the glow are shader
//! parameters (distances in atlas pixels) carried per vertex.

use glam::{Vec2, Vec4};

use super::atlas::FontAtlases;
use super::sdf::{BASE_SIZE, SPREAD};
use crate::components::TextComponent;
use crate::ui::text::{layout_text, PlacedGlyph};

/// Synthesized italic's shear (x per unit of height above the baseline, ≈ 11°).
const ITALIC_SHEAR: f32 = 0.2;
/// Synthesized bold's dilation, in ems (per side).
const FAUX_BOLD_DILATE: f32 = 0.02;

/// One glyph quad.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextQuad {
    /// The atlas (font path) it samples.
    pub(crate) font: Option<String>,
    /// Bottom-left, top-left, top-right, bottom-right; rect-local reference units.
    pub(crate) corners: [Vec2; 4],
    /// Texture coordinates `(min, max)`, v down.
    pub(crate) uv: (Vec2, Vec2),
    pub(crate) fill: Vec4,
    pub(crate) outline: Vec4,
    pub(crate) glow: Vec4,
    /// `[1 (SDF), dilate, outline width, glow reach]`, atlas pixels.
    pub(crate) sdf: [f32; 4],
}

/// The quads drawing `text` in a `size` rect (shadow first).
pub(crate) fn text_quads(
    text: &TextComponent,
    size: Vec2,
    atlases: &mut FontAtlases,
) -> Vec<TextQuad> {
    let layout = layout_text(text, size);
    let mut main = Vec::with_capacity(layout.glyphs.len());
    for g in &layout.glyphs {
        let font = &layout.faces[g.face];
        let Some((atlas, uv)) = atlases.glyph(font, g.glyph) else {
            continue;
        };
        let scale = g.size / BASE_SIZE;
        let lo = g.origin + atlas.origin * scale;
        let hi = lo + Vec2::new(atlas.texels[2] as f32, atlas.texels[3] as f32) * scale;
        let sdf = effects(text, g);
        let unless_zero = |width: f32, c: Vec4| if width > 0.0 { c } else { Vec4::ZERO };
        main.push(TextQuad {
            font: font.clone(),
            corners: corners(lo, hi, g),
            uv,
            fill: g.color,
            outline: unless_zero(sdf[2], text.outline_color),
            glow: unless_zero(sdf[3], text.glow_color),
            sdf,
        });
    }
    let shadow = text.shadow_offset != Vec2::ZERO && text.shadow_color.w > 0.0;
    let shadows = main.iter().filter(|_| shadow).map(|q| TextQuad {
        corners: q.corners.map(|c| c + text.shadow_offset),
        fill: text.shadow_color,
        outline: if q.sdf[2] > 0.0 {
            text.shadow_color
        } else {
            Vec4::ZERO
        },
        glow: Vec4::ZERO,
        sdf: [q.sdf[0], q.sdf[1], q.sdf[2], 0.0],
        ..q.clone()
    });
    shadows
        .collect::<Vec<_>>()
        .into_iter()
        .chain(main)
        .collect()
}

/// The quad over `(lo, hi)`, sheared for synthesized italic.
fn corners(lo: Vec2, hi: Vec2, g: &PlacedGlyph) -> [Vec2; 4] {
    let shear = |p: Vec2| {
        let dx = if g.faux_italic {
            (p.y - g.origin.y) * ITALIC_SHEAR
        } else {
            0.0
        };
        Vec2::new(p.x + dx, p.y)
    };
    [
        shear(lo),
        shear(Vec2::new(lo.x, hi.y)),
        shear(hi),
        shear(Vec2::new(hi.x, lo.y)),
    ]
}

/// `[1, dilate, outline, glow]` in atlas pixels, the total kept inside the field's
/// reach. A zero width also zeroes that effect's colour (see [`text_quads`]), so
/// no fringe of it shows at the edge.
fn effects(text: &TextComponent, g: &PlacedGlyph) -> [f32; 4] {
    let reach = SPREAD - 1.0;
    let dilate = if g.faux_bold {
        FAUX_BOLD_DILATE * BASE_SIZE
    } else {
        0.0
    };
    let outline = (text.outline_width * BASE_SIZE).min(reach - dilate);
    let glow = (text.glow_size * BASE_SIZE).min(reach - dilate - outline);
    [1.0, dilate, outline, glow]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_becomes_one_quad_per_visible_glyph_inside_its_rect() {
        let mut atlases = FontAtlases::default();
        let text = TextComponent {
            text: "Hi there".into(),
            font_size: 40.0,
            ..Default::default()
        };
        let rect = Vec2::new(400.0, 100.0);
        let quads = text_quads(&text, rect, &mut atlases);
        assert_eq!(quads.len(), 7, "the space draws nothing");
        for q in &quads {
            assert!(q.corners[0].x >= -SPREAD && q.corners[2].y <= rect.y + SPREAD);
            assert!(q.uv.0.x < q.uv.1.x && q.uv.0.y < q.uv.1.y);
        }
    }

    #[test]
    fn a_shadow_doubles_the_quads_and_draws_first() {
        let mut atlases = FontAtlases::default();
        let text = TextComponent {
            text: "<i>A</i>".into(),
            shadow_offset: Vec2::new(3.0, -3.0),
            outline_width: 10.0,
            glow_size: 10.0,
            ..Default::default()
        };
        let quads = text_quads(&text, Vec2::new(300.0, 100.0), &mut atlases);
        assert_eq!(quads.len(), 2);
        let (shadow, glyph) = (&quads[0], &quads[1]);
        assert_eq!(shadow.corners[0], glyph.corners[0] + Vec2::new(3.0, -3.0));
        assert_eq!(shadow.fill, text.shadow_color);
        assert!(
            glyph.corners[1].x > glyph.corners[0].x,
            "italic leans right"
        );
        assert!(
            glyph.sdf[2] + glyph.sdf[3] <= SPREAD,
            "effects stay in the field"
        );
    }
}
