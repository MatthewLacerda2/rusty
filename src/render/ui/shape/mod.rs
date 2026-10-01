//! src/render/ui/shape/ — a `Shape` (#425) → SDF quads in the canvas mesh.
//!
//! A shape is drawn by the fragment shader from a signed distance field, so its
//! geometry is just quads over the element's rect: one for the shape itself
//! (grown by an antialiasing margin), preceded by one for its drop shadow (offset,
//! grown by the blur) and one for its outer glow (grown by the glow's reach) — the
//! effects are drawn under the shape. Every quad carries the same rect-local
//! frame (`local`: position from the rect's centre and the half size), so the
//! shader evaluates the same field on all three; only the mode parameters differ.
//! The quads map through the element's laid-out corners, so rotation and scale
//! carry the shape, and its vertices are solid (white texture), so shapes batch
//! with solid images.
//!
//! Shape kind codes (`UiVertex::shape[0]`) and parameters, matching `ui.wgsl`:
//! `0` rounded rect, `1` chamfered rect (`radii` per corner), `2` ellipse, `3` ring
//! (`[inner radius, arc start, arc end]`), `4` line (`[thickness, dash, gap]`).

use glam::{Vec2, Vec4};

use super::mesh::{close_batch, to_ndc, visible_clip, CanvasMesh, Frame, Inherited, UiSource};
use super::vertex::{Fill, MODE_SHAPE};
use crate::components::{ShapeComponent, ShapeCorner, ShapeKind};
use crate::ui::UiRect;

/// A quad's colours: its fill and its border's (both tinted).
#[derive(Clone, Copy)]
struct Paint {
    fill: Fill,
    border: Vec4,
}

/// What one quad of a shape draws.
#[derive(Clone, Copy)]
enum Layer {
    /// The soft copy: offset, edge softened over `blur` either side.
    Shadow { offset: Vec2, blur: f32 },
    /// The halo reaching `reach` past the edge.
    Glow { reach: f32 },
    /// The shape: fill and border.
    Body,
}

/// Emit `id`'s Shape (if any, visible and not fully clipped) into `mesh`: shadow,
/// glow, then the shape.
pub(in crate::render::ui) fn push_shape(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    id: u32,
    rect: &UiRect,
    state: Inherited,
) {
    let Some(shape) = frame.world.shape(id) else {
        return;
    };
    let Some(clip) = visible_clip(state, state.alpha, frame.screen) else {
        return;
    };
    // A Selectable's ColorTint (#420) multiplies every colour, like the group alpha.
    let tint = shape.state_tint * Vec4::new(1.0, 1.0, 1.0, state.alpha);
    let start = mesh.vertices.len() as u32;
    if shape.shadow.is_on() {
        let s = shape.shadow;
        let layer = Layer::Shadow {
            offset: s.offset,
            blur: s.blur,
        };
        let fill = Fill::new(None, s.color * tint);
        push_quad(
            mesh,
            frame,
            rect,
            &shape,
            layer,
            Paint {
                fill,
                border: Vec4::ZERO,
            },
        );
    }
    if shape.glow.is_on() {
        let g = shape.glow;
        let color = (g.color.truncate() * g.intensity).extend(g.color.w);
        let layer = Layer::Glow { reach: g.size };
        let fill = Fill::new(None, color * tint);
        push_quad(
            mesh,
            frame,
            rect,
            &shape,
            layer,
            Paint {
                fill,
                border: Vec4::ZERO,
            },
        );
    }
    let flat = shape.gradient.is_none().then_some(shape.color);
    let fill = Fill::new(shape.gradient.as_ref(), flat.unwrap_or(Vec4::ONE) * tint);
    let border = shape.border_color * tint;
    push_quad(
        mesh,
        frame,
        rect,
        &shape,
        Layer::Body,
        Paint { fill, border },
    );
    close_batch(mesh, (UiSource::Solid, shape.blend), clip, start);
}

/// Append one quad (two triangles) of `layer` over `rect`.
fn push_quad(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    rect: &UiRect,
    shape: &ShapeComponent,
    layer: Layer,
    paint: Paint,
) {
    let size = rect.rect.1;
    let half = size * 0.5;
    let [bl, tl, _, br] = rect.corners;
    let axis = |v: Vec2, len: f32| if len > 1e-6 { v / len } else { Vec2::ZERO };
    let (ax, ay) = (axis(br - bl, size.x), axis(tl - bl, size.y));
    let centre = bl + (br - bl) * 0.5 + (tl - bl) * 0.5;
    // One screen pixel of antialiasing margin, in reference units.
    let aa = 1.5 / rect.scale_factor.max(1e-3);
    let (offset, grow, sdf) = match layer {
        Layer::Shadow { offset, blur } => (offset, blur + aa, [MODE_SHAPE, 0.0, blur, 0.0]),
        Layer::Glow { reach } => (Vec2::ZERO, reach + aa, [MODE_SHAPE, 0.0, 0.0, reach]),
        Layer::Body => (Vec2::ZERO, aa, [MODE_SHAPE, shape.border_width, 0.0, 0.0]),
    };
    let ext = half + Vec2::splat(grow);
    let corners = [
        Vec2::new(-ext.x, -ext.y),
        Vec2::new(-ext.x, ext.y),
        Vec2::new(ext.x, ext.y),
        Vec2::new(ext.x, -ext.y),
    ];
    let (kind, params) = kind_params(shape);
    mesh.vertices.extend([0, 1, 2, 0, 2, 3].map(|i| {
        let l = corners[i];
        let pos = to_ndc(
            centre + ax * (l.x + offset.x) + ay * (l.y + offset.y),
            rect,
            frame.screen,
        );
        let mut v = paint
            .fill
            .vertex(pos, [0.0, 0.0], [l.x, l.y, half.x, half.y]);
        v.sdf = sdf;
        v.outline = paint.border.to_array();
        v.shape = [kind, params[0], params[1], params[2]];
        v.radii = shape.radius.to_array();
        v
    }));
}

/// The shape's kind code and its three parameters (see the module docs).
fn kind_params(shape: &ShapeComponent) -> (f32, [f32; 3]) {
    match shape.kind {
        ShapeKind::Rect => match shape.corner {
            ShapeCorner::Round => (0.0, [0.0; 3]),
            ShapeCorner::Chamfer => (1.0, [0.0; 3]),
        },
        ShapeKind::Ellipse => (2.0, [0.0; 3]),
        ShapeKind::Ring => (3.0, [shape.inner_radius, shape.arc_start, shape.arc_end]),
        ShapeKind::Line => (4.0, [shape.thickness, shape.dash, shape.gap]),
    }
}

#[cfg(test)]
mod tests;
