//! src/render/ui/mesh.rs — the laid-out UI → one vertex list + batches per canvas (#418).
//!
//! GPU-free: [`build_canvas_meshes`] walks the layout in draw order (canvas
//! `sort_order`, then hierarchy pre-order) and emits every visible `Image` and
//! `Shape` and `Text` (an entity with several draws them in that order) as triangles in
//! normalized device coordinates, tagged with what it samples and how it is
//! clipped. Consecutive graphics sharing a source (solid, a texture, a font's
//! atlas), a blend mode (#425), a clip ([`UiClip`]: rect, feather and mask, #428),
//! a custom shader (#427, `custom::shaded`) and no backdrop merge into one [`UiBatch`]; a change of any starts a new one —
//! so a HUD of solid bars is one draw call per canvas however many bars it has,
//! and a label is one draw call however many glyphs it has.
//!
//! Inherited state rides down the walk (parents precede children in the layout):
//! **visibility** (the entity and every ancestor active), **alpha** (the product of
//! every `CanvasGroup` on the chain) and **clip** (every `RectMask` on the chain
//! intersected, with its feather, and the nearest `Mask` — see `clip`).
//!
//! Two graphics are not plain batches: a `Mask`'s graphic is recorded as a
//! [`MaskDraw`] (rendered into its coverage texture, and batched only when
//! `show_mask_graphic` is on), and a `BackdropFilter` adds a backdrop batch under
//! the graphic on an overlay canvas (`effects`). Both take their shape from
//! [`push_graphic`]: the entity's Image, else its Shape, else its rect.
//!
//! A `WorldSpace` canvas (#429) has no screen: its "screen" is its own reference
//! rect (scale 1), so its NDC span exactly that rect and the world pass maps them
//! onto the canvas plane. Its clips are rects in that space too; the world pass
//! cannot scissor a plane in perspective, so it clips them per fragment against
//! [`UiClip::ndc`] instead (#619).

use std::collections::HashMap;

use glam::{Vec2, Vec4};

pub(super) use super::clip::Inherited;
pub(crate) use super::clip::UiClip;
use super::custom::shaded::{shade, UiShade};
use super::effects::backdrop::{push_backdrop, UiBackdrop};
use super::effects::mask::{push_mask, MaskDraw};
use super::geometry::image_triangles;
use super::shape::{push_shape, push_shape_body};
use super::text::atlas::FontAtlases;
use super::text::emit::push_text;
pub(crate) use super::vertex::UiVertex;
use super::vertex::{Fill, MODE_IMAGE};
use crate::components::{ImageComponent, UiBlend};
use crate::ecs::World;
use crate::ui::{CanvasSpace, UiLayout, UiRect};

/// What a batch samples.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UiSource {
    /// Nothing: the white texture, a solid colour.
    Solid,
    /// An image texture, by path.
    Texture(String),
    /// A font's SDF atlas, by font path (`None`: the default font).
    Font(Option<String>),
}

/// A run of vertices drawn with one source, one blend mode, one clip and one
/// backdrop.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiBatch {
    /// What it samples.
    pub(crate) source: UiSource,
    /// How it composites (#425): each mode is its own pipeline.
    pub(crate) blend: UiBlend,
    /// How it is clipped.
    pub(crate) clip: UiClip,
    /// The custom ui shader it draws with (#427), or `None` for the standard one.
    pub(crate) shade: Option<UiShade>,
    /// A backdrop batch (#426): what it shows behind the graphic, or `None` for an
    /// ordinary graphic.
    pub(crate) backdrop: Option<UiBackdrop>,
    /// The vertex range in the canvas's vertex list.
    pub(crate) range: std::ops::Range<u32>,
}

/// One canvas's geometry for this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasMesh {
    pub(crate) canvas: u32,
    /// The pixel frame it was meshed into: the screen, or a `WorldSpace` canvas's
    /// own reference rect — what its NDC and scissors are relative to.
    pub(crate) frame: Vec2,
    pub(crate) vertices: Vec<UiVertex>,
    pub(crate) batches: Vec<UiBatch>,
    /// Its `Mask` graphics, parents before children (#428).
    pub(crate) masks: Vec<MaskDraw>,
}

/// What every element of one build reads: the scene, the target size in pixels and
/// the texture-size lookup.
pub(super) struct Frame<'a> {
    pub(super) world: &'a World,
    pub(super) screen: Vec2,
    tex_size: &'a dyn Fn(&str) -> Option<Vec2>,
    /// Whether the canvas being built is a screen overlay (backdrops draw only there).
    pub(super) overlay: bool,
}

/// Build every canvas's mesh on a `screen`-pixel target. `tex_size` reports a
/// texture's size in texels (`None` when not loaded); text glyphs are packed into
/// `atlases` as they are first drawn.
pub(crate) fn build_canvas_meshes(
    world: &World,
    layout: &UiLayout,
    screen: Vec2,
    tex_size: &dyn Fn(&str) -> Option<Vec2>,
    atlases: &mut FontAtlases,
) -> Vec<CanvasMesh> {
    let screen = screen.max(Vec2::ONE);
    let mut frame = Frame {
        world,
        screen,
        tex_size,
        overlay: true,
    };
    let mut meshes: Vec<CanvasMesh> = Vec::new();
    let mut state: HashMap<u32, Inherited> = HashMap::new();
    for (id, rect) in layout.iter() {
        let parent = world
            .parent_id(id)
            .and_then(|p| state.get(&p).copied())
            .filter(|_| id != rect.canvas)
            .unwrap_or_else(|| Inherited::root(world, id));
        let (here, down) = parent.child(world, id, rect);
        state.insert(id, down);
        if meshes.last().map(|m| m.canvas) != Some(rect.canvas) {
            frame.screen = canvas_screen(world, rect.canvas, screen);
            frame.overlay = layout.space(rect.canvas) == CanvasSpace::Screen;
            meshes.push(CanvasMesh {
                canvas: rect.canvas,
                frame: frame.screen,
                ..Default::default()
            });
        }
        if let Some(mesh) = meshes.last_mut() {
            push_backdrop(mesh, &frame, id, rect, here);
            if world.mask(id).is_none_or(|m| m.show_mask_graphic) {
                push_image(mesh, &frame, id, rect, here);
                push_shape(mesh, &frame, id, rect, here);
            }
            push_mask(mesh, &frame, id, rect, here);
            push_text(mesh, &frame, id, rect, here, atlases);
        }
    }
    meshes.retain(|m| !m.batches.is_empty());
    meshes
}

/// The pixel frame canvas `id` draws into: the screen, or a `WorldSpace` canvas's
/// own reference rect.
fn canvas_screen(world: &World, id: u32, screen: Vec2) -> Vec2 {
    match world.canvas(id) {
        Some(c) if c.is_world_space() => c.size(screen),
        _ => screen,
    }
}

/// Emit `id`'s Image (if any, visible and not fully clipped) into `mesh`.
fn push_image(mesh: &mut CanvasMesh, frame: &Frame, id: u32, rect: &UiRect, state: Inherited) {
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
    let tex = image.shown_texture().and_then(frame.tex_size);
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
pub(super) enum Coverage {
    /// The graphic's own colour (or gradient) — a Mask: its alpha is the clip.
    Own,
    /// Opaque white at this alpha whatever the graphic's colour — a backdrop.
    Flat(f32),
}

/// Append the triangles of `id`'s graphic shape — its Image, else its Shape, else
/// its whole rect — painted per `coverage`, and return what they sample. A Mask's
/// coverage and a backdrop both take their shape from here (#426, #428).
pub(super) fn push_graphic(
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

/// The clip a graphic draws under, or `None` when it draws nothing (hidden, fully
/// transparent, or clipped away entirely).
pub(super) fn visible_clip(state: Inherited, alpha: f32, screen: Vec2) -> Option<UiClip> {
    state.visible_clip(alpha, screen)
}

/// A canvas point (reference units) → normalized device coordinates.
pub(super) fn to_ndc(canvas: Vec2, rect: &UiRect, screen: Vec2) -> [f32; 2] {
    (canvas * rect.scale_factor / screen * 2.0 - Vec2::ONE).to_array()
}

/// Close the vertices from `start` on into the last batch when it shares `source`,
/// blend mode, `shade` and `clip`, else into a new one.
pub(super) fn close_batch(
    mesh: &mut CanvasMesh,
    (source, blend, shade): (UiSource, UiBlend, &Option<UiShade>),
    clip: UiClip,
    start: u32,
) {
    close_run(mesh, (source, blend, None, shade.as_ref()), clip, start);
}

/// [`close_batch`] for a run drawn with `backdrop` (`None`: an ordinary graphic).
pub(super) fn close_run(
    mesh: &mut CanvasMesh,
    (source, blend, backdrop, shade): (UiSource, UiBlend, Option<UiBackdrop>, Option<&UiShade>),
    clip: UiClip,
    start: u32,
) {
    let end = mesh.vertices.len() as u32;
    let key = (&source, blend, clip, backdrop, shade);
    let same = |b: &UiBatch| (&b.source, b.blend, b.clip, b.backdrop, b.shade.as_ref()) == key;
    match mesh.batches.last_mut() {
        Some(b) if b.range.end == start && same(b) => b.range.end = end,
        _ if end > start => mesh.batches.push(UiBatch {
            shade: shade.cloned(),
            source,
            blend,
            clip,
            backdrop,
            range: start..end,
        }),
        _ => {}
    }
}

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod mesh_tests;
