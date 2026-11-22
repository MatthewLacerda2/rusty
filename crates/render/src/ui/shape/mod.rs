use glam::{Vec2, Vec4};

use super::custom::shaded::shade;
use super::mesh::{close_batch, to_ndc, visible_clip, CanvasMesh, Frame, Inherited, UiSource};
use super::vertex::{Fill, MODE_SHAPE};
use rusty_components::{ShapeComponent, ShapeCorner, ShapeKind};
use rusty_ui::UiRect;

#[derive(Clone, Copy)]
struct Paint {
    fill: Fill,
    border: Vec4,
}

#[derive(Clone, Copy)]
enum Layer {
    Shadow { offset: Vec2, blur: f32 },
    Glow { reach: f32 },
    Body,
}

pub(in crate::ui) fn push_shape(
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
    let tint = shape.state_tint * Vec4::new(1.0, 1.0, 1.0, state.alpha);
    let start = mesh.vertices.len() as u32;
    push_effects(mesh, frame, rect, &shape, tint);
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
    let shade = shade(&shape.shader, rect, frame.screen);
    close_batch(mesh, (UiSource::Solid, shape.blend, &shade), clip, start);
}

pub(in crate::ui) fn push_shape_body(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    rect: &UiRect,
    shape: &ShapeComponent,
    fill: Fill,
) {
    let border = Vec4::new(1.0, 1.0, 1.0, fill.max_alpha());
    push_quad(
        mesh,
        frame,
        rect,
        shape,
        Layer::Body,
        Paint { fill, border },
    );
}

fn push_effects(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    rect: &UiRect,
    shape: &ShapeComponent,
    tint: Vec4,
) {
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
            shape,
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
            shape,
            layer,
            Paint {
                fill,
                border: Vec4::ZERO,
            },
        );
    }
}

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
    // Antialiasing margin: 1.5 screen pixels, in reference units.
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

/// Kind codes 0-4 are the branches of `ui.wgsl`'s `shape_distance`: change both together.
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
