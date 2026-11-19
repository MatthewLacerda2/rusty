//! crates/render/src/ui/mesh/graphic.rs — an `Image`'s triangles in the canvas mesh,
//! and the graphic shape a Mask's coverage and a backdrop take (#418, #426, #428).

use glam::Vec4;

use super::{close_batch, to_ndc, visible_clip, CanvasMesh, Frame, Inherited, UiSource};
use crate::ui::custom::shaded::shade;
use crate::ui::geometry::image_triangles;
use crate::ui::shape::push_shape_body;
use crate::ui::vertex::{Fill, MODE_IMAGE};
use rusty_components::ImageComponent;
use rusty_ui::UiRect;

/// Emit `id`'s Image (if any, visible and not fully clipped) into `mesh`.
pub(super) fn push_image(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    id: u32,
    rect: &UiRect,
    state: Inherited,
) {
    let Some(image) = frame.world.image(id) else {
        return;
    };
    // A Selectable's ColorTint (#420) multiplies in, like the group alpha.
    let tint = image.state_tint * Vec4::new(1.0, 1.0, 1.0, state.alpha);
    let flat = image.gradient.is_none().then_some(image.color);
    let fill = Fill::new(image.gradient.as_ref(), flat.unwrap_or(Vec4::ONE) * tint);
    let Some(clip) = visible_clip(state, fill.max_alpha(), frame.screen) else {
        return;
    };
    let start = mesh.vertices.len() as u32;
    let source = push_image_tris(mesh, frame, rect, &image, fill);
    let shade = shade(&image.shader, rect, frame.screen);
    close_batch(mesh, (source, image.blend, &shade), clip, start);
}

/// Append `image`'s triangles over `rect` painted `fill`; returns what they sample.
fn push_image_tris(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    rect: &UiRect,
    image: &ImageComponent,
    fill: Fill,
) -> UiSource {
    let size = rect.rect.1;
    let tex = image
        .shown_texture()
        .and_then(|t| (frame.tex_size)(t.as_str()));
    let tris = image_triangles(image, size, tex);
    let [bl, tl, _, br] = rect.corners;
    let half = size * 0.5;
    mesh.vertices.extend(tris.iter().map(|&(p, uv)| {
        let pos = to_ndc(bl + (br - bl) * p.x + (tl - bl) * p.y, rect, frame.screen);
        let local = ((p - 0.5) * size).extend(half.x).extend(half.y);
        let mut v = fill.vertex(pos, [uv.x, 1.0 - uv.y], local.to_array());
        v.sdf[0] = MODE_IMAGE;
        v
    }));
    image
        .shown_texture()
        .map_or(UiSource::Solid, |t| UiSource::Texture(t.to_string()))
}

/// What a [`push_graphic`] shape is painted with.
#[derive(Clone, Copy)]
pub(in crate::ui) enum Coverage {
    /// The graphic's own colour (or gradient) — a Mask: its alpha is the clip.
    Own,
    /// Opaque white at this alpha whatever the graphic's colour — a backdrop.
    Flat(f32),
}

/// Append the triangles of `id`'s graphic shape — its Image, else its Shape, else
/// its whole rect — painted per `coverage`, and return what they sample. A Mask's
/// coverage and a backdrop both take their shape from here (#426, #428).
pub(in crate::ui) fn push_graphic(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    id: u32,
    rect: &UiRect,
    coverage: Coverage,
) -> UiSource {
    let paint = |gradient, color: Vec4| match coverage {
        Coverage::Own => Fill::new(gradient, color),
        Coverage::Flat(a) => Fill::new(None, Vec4::new(1.0, 1.0, 1.0, a)),
    };
    if let Some(image) = frame.world.image(id) {
        let fill = paint(image.gradient.as_ref(), image.color);
        return push_image_tris(mesh, frame, rect, &image, fill);
    }
    if let Some(shape) = frame.world.shape(id) {
        push_shape_body(
            mesh,
            frame,
            rect,
            &shape,
            paint(shape.gradient.as_ref(), shape.color),
        );
        return UiSource::Solid;
    }
    let fill = paint(None, Vec4::ONE);
    push_image_tris(mesh, frame, rect, &ImageComponent::default(), fill)
}
