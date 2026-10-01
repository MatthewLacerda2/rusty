//! src/render/ui/custom/shaded.rs — a graphic's custom ui shader, as its batch carries it
//! (#427).
//!
//! GPU-free. An `Image`, `Shape` or `Text` naming a [`UiShader`] draws in batches tagged with
//! a [`UiShade`]: the shader and its param values, plus the graphic's rect — what
//! the variant's per-graphic uniform is filled from (`uniforms`). Batches merge only
//! when their shades are equal, so one graphic's glyphs stay one draw, and two
//! shaded graphics share a draw only when everything their uniform holds matches.

use glam::Vec2;

use crate::components::UiShader;
use crate::render::ui::mesh::to_ndc;
use crate::ui::UiRect;

/// The shade a batch draws with. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiShade {
    /// The shader's name and the graphic's runtime param values.
    pub(crate) shader: UiShader,
    /// The graphic's rect in its canvas's NDC: its bottom-left corner, then the
    /// vectors along its bottom and left edges (rotation and scale included).
    pub(crate) rect: [[f32; 2]; 3],
    /// The rect's size in pixels of the frame it is meshed into (the screen, or a
    /// world canvas's own reference rect).
    pub(crate) size: Vec2,
}

/// The shade for a graphic with shader slot `shader` laid out at `rect` on a
/// `screen`-pixel frame; `None` draws with the standard shader.
pub(in crate::render::ui) fn shade(
    shader: &Option<UiShader>,
    rect: &UiRect,
    screen: Vec2,
) -> Option<UiShade> {
    let shader = shader.as_ref().filter(|s| !s.name.is_empty())?;
    let [bl, tl, _, br] = rect.corners;
    let [o, x, y] = [bl, br, tl].map(|c| Vec2::from(to_ndc(c, rect, screen)));
    Some(UiShade {
        shader: shader.clone(),
        rect: [o.to_array(), (x - o).to_array(), (y - o).to_array()],
        size: rect.rect.1 * rect.scale_factor,
    })
}
