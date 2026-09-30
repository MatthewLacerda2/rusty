//! src/render/ui/text/emit.rs — a `Text` element's quads → the canvas mesh (#419).
//!
//! The text counterpart of the mesh builder's image emission: each glyph quad
//! (rect-local) is mapped through the element's laid-out corners — so rotation and
//! scale carry the text — and appended as two triangles, batched per font atlas
//! under the element's inherited clip, with its CanvasGroup alpha folded into the
//! fill, outline and glow colours.

use glam::{Vec2, Vec4};

use super::atlas::FontAtlases;
use super::quads::{text_quads, TextQuad};
use crate::render::ui::mesh::{
    close_batch, to_ndc, visible_clip, CanvasMesh, Frame, Inherited, UiSource, UiVertex,
};
use crate::ui::UiRect;

/// Emit `id`'s Text (if any, visible and not fully clipped) into `mesh`.
pub(in crate::render::ui) fn push_text(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    id: u32,
    rect: &UiRect,
    state: Inherited,
    atlases: &mut FontAtlases,
) {
    let Some(text) = frame.world.text(id) else {
        return;
    };
    let Some(clip) = visible_clip(state, state.alpha, frame.screen) else {
        return;
    };
    let size = rect.rect.1;
    let [bl, tl, _, br] = rect.corners;
    let axis = |v: Vec2, len: f32, unit: Vec2| if len > 1e-6 { v / len } else { unit };
    let (ax, ay) = (
        axis(br - bl, size.x, Vec2::X),
        axis(tl - bl, size.y, Vec2::Y),
    );
    // A Selectable's ColorTint (#420) multiplies every colour, like the group alpha.
    let alpha = text.state_tint * Vec4::new(1.0, 1.0, 1.0, state.alpha);
    for quad in text_quads(&text, size, atlases) {
        let start = mesh.vertices.len() as u32;
        let TextQuad { corners, uv, .. } = &quad;
        let uvs = [
            Vec2::new(uv.0.x, uv.1.y),
            uv.0,
            Vec2::new(uv.1.x, uv.0.y),
            uv.1,
        ];
        mesh.vertices.extend([0, 1, 2, 0, 2, 3].map(|i| UiVertex {
            pos: to_ndc(
                bl + ax * corners[i].x + ay * corners[i].y,
                rect,
                frame.screen,
            ),
            uv: uvs[i].to_array(),
            color: (quad.fill * alpha).to_array(),
            outline: (quad.outline * alpha).to_array(),
            glow: (quad.glow * alpha).to_array(),
            sdf: quad.sdf,
        }));
        close_batch(mesh, UiSource::Font(quad.font), clip, start);
    }
}
