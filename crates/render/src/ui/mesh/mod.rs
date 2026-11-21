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

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UiSource {
    Solid,
    Texture(String),
    Font(Option<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiBatch {
    pub(crate) source: UiSource,
    pub(crate) blend: UiBlend,
    pub(crate) clip: UiClip,
    pub(crate) shade: Option<UiShade>,
    pub(crate) backdrop: Option<UiBackdrop>,
    pub(crate) range: std::ops::Range<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasMesh {
    pub(crate) canvas: u32,
    pub(crate) frame: Vec2,
    pub(crate) vertices: Vec<UiVertex>,
    pub(crate) batches: Vec<UiBatch>,
    pub(crate) masks: Vec<MaskDraw>,
}

pub(crate) type TexSize<'a> = &'a dyn Fn(&str) -> Option<Vec2>;

pub(crate) type Sources<'a, 'b> = (TexSize<'a>, &'b mut FontAtlases);

pub(super) struct Frame<'a> {
    pub(super) world: &'a World,
    pub(super) screen: Vec2,
    tex_size: TexSize<'a>,
    pub(super) overlay: bool,
}

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

pub(super) fn visible_clip(state: Inherited, alpha: f32, screen: Vec2) -> Option<UiClip> {
    state.visible_clip(alpha, screen)
}

pub(super) fn to_ndc(canvas: Vec2, rect: &UiRect, screen: Vec2) -> [f32; 2] {
    (canvas * rect.scale_factor / screen * 2.0 - Vec2::ONE).to_array()
}

pub(super) fn close_batch(
    mesh: &mut CanvasMesh,
    (source, blend, shade): (UiSource, UiBlend, &Option<UiShade>),
    clip: UiClip,
    start: u32,
) {
    close_run(mesh, (source, blend, None, shade.as_ref()), clip, start);
}

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
