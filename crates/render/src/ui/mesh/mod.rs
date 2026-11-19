//! crates/render/src/ui/mesh/ — the laid-out UI → one vertex list + batches per canvas (#418).
//!
//! GPU-free: [`build_canvas`] walks one canvas's run of the layout in draw order
//! (hierarchy pre-order; canvases follow `sort_order`, `kept`) and emits every visible `Image` and
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

use glam::Vec2;

pub(super) use super::clip::Inherited;
pub(crate) use super::clip::UiClip;

mod graphic;
use super::custom::shaded::UiShade;
use super::effects::backdrop::{push_backdrop, UiBackdrop};
use super::effects::mask::{push_mask, MaskDraw};
#[cfg(test)]
pub(crate) use super::kept::build_canvas_meshes;
use super::shape::push_shape;
use super::text::atlas::FontAtlases;
use super::text::emit::push_text;
pub(crate) use super::vertex::UiVertex;
use graphic::push_image;
pub(in crate::ui) use graphic::{push_graphic, Coverage};
use rusty_components::UiBlend;
use rusty_ecs::World;
use rusty_ui::{CanvasSpace, UiLayout, UiRect};

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

/// A texture's source size in texels by path (`None` when not loaded).
pub(crate) type TexSize<'a> = &'a dyn Fn(&str) -> Option<Vec2>;

/// What a canvas build reads besides the World: texture sizes, and the glyph
/// atlases it packs into.
pub(crate) type Sources<'a, 'b> = (TexSize<'a>, &'b mut FontAtlases);

/// What every element of one build reads: the scene, the target size in pixels and
/// the texture-size lookup.
pub(super) struct Frame<'a> {
    pub(super) world: &'a World,
    pub(super) screen: Vec2,
    tex_size: TexSize<'a>,
    /// Whether the canvas being built is a screen overlay (backdrops draw only there).
    pub(super) overlay: bool,
}

/// The pixel frame canvas `canvas` is meshed into on a `screen`-pixel target (the
/// screen, or a `WorldSpace` canvas's own reference rect), and whether it is a
/// screen overlay.
pub(super) fn canvas_frame(
    world: &World,
    layout: &UiLayout,
    canvas: u32,
    screen: Vec2,
) -> (Vec2, bool) {
    let screen = screen.max(Vec2::ONE);
    let frame = match world.canvas(canvas) {
        Some(c) if c.is_world_space() => c.size(screen),
        _ => screen,
    };
    (frame, layout.space(canvas) == CanvasSpace::Screen)
}

/// One canvas's mesh from its `run` of laid-out elements (non-empty, draw order).
pub(super) fn build_canvas(
    world: &World,
    layout: &UiLayout,
    run: &[(u32, &UiRect)],
    screen: Vec2,
    (tex_size, atlases): Sources,
) -> CanvasMesh {
    let canvas = run.first().map_or(0, |(_, r)| r.canvas);
    let (screen, overlay) = canvas_frame(world, layout, canvas, screen);
    let frame = Frame {
        world,
        screen,
        tex_size,
        overlay,
    };
    let mut mesh = CanvasMesh {
        canvas,
        frame: screen,
        ..Default::default()
    };
    let mut state: HashMap<u32, Inherited> = HashMap::new();
    for &(id, rect) in run {
        let parent = world
            .parent_id(id)
            .and_then(|p| state.get(&p).copied())
            .filter(|_| id != rect.canvas)
            .unwrap_or_else(|| Inherited::root(world, id));
        let (here, down) = parent.child(world, id, rect);
        state.insert(id, down);
        push_backdrop(&mut mesh, &frame, id, rect, here);
        if world.mask(id).is_none_or(|m| m.show_mask_graphic) {
            push_image(&mut mesh, &frame, id, rect, here);
            push_shape(&mut mesh, &frame, id, rect, here);
        }
        push_mask(&mut mesh, &frame, id, rect, here);
        push_text(&mut mesh, &frame, id, rect, here, atlases);
    }
    mesh
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
mod tests;
