//! src/render/ui/mesh.rs — the laid-out UI → one vertex list + batches per canvas (#418).
//!
//! GPU-free: [`build_canvas_meshes`] walks the layout in draw order (canvas
//! `sort_order`, then hierarchy pre-order) and emits every visible `Image` and
//! `Text` (an entity with both draws the image, then the text) as triangles in
//! normalized device coordinates, tagged with what it samples and how it is
//! clipped. Consecutive graphics sharing a source (solid, a texture, a font's
//! atlas) and a clip merge into one [`UiBatch`]; a change of either starts a new
//! one — so a HUD of solid bars is one draw call per canvas however many bars it
//! has, and a label is one draw call however many glyphs it has.
//!
//! Inherited state rides down the walk (parents precede children in the layout):
//! **visibility** (the entity and every ancestor active), **alpha** (the product of
//! every `CanvasGroup` on the chain) and **clip** (every `RectMask` on the chain,
//! intersected, as a pixel scissor rect).
//!
//! A `WorldSpace` canvas (#429) has no screen: its "screen" is its own reference
//! rect (scale 1), so its NDC span exactly that rect and the world pass maps them
//! onto the canvas plane. Its clips are rects in that space too; the world pass
//! cannot scissor them, so they only cull graphics wholly outside a mask.

use std::collections::HashMap;

use bytemuck::Zeroable;
use glam::{Vec2, Vec4};

use super::geometry::image_triangles;
use super::text::atlas::FontAtlases;
use super::text::emit::push_text;
use crate::ecs::World;
use crate::ui::{UiLayout, UiRect};

/// One UI vertex: NDC position, texture coordinate (v down, as the GPU samples) and
/// the straight-alpha display-space tint (group alpha folded in). Text vertices
/// also carry their outline and glow colours and the SDF parameters
/// (`[1, dilate, outline, glow]` in atlas pixels); an image's are zero.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct UiVertex {
    pub(crate) pos: [f32; 2],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) glow: [f32; 4],
    pub(crate) sdf: [f32; 4],
}

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

/// A scissor rect in target pixels, top-left origin (wgpu's convention).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scissor {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

/// A run of vertices drawn with one source and one clip.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiBatch {
    /// What it samples.
    pub(crate) source: UiSource,
    /// The scissor, or `None` for the whole target.
    pub(crate) clip: Option<Scissor>,
    /// The vertex range in the canvas's vertex list.
    pub(crate) range: std::ops::Range<u32>,
}

/// One canvas's geometry for this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasMesh {
    pub(crate) canvas: u32,
    pub(crate) vertices: Vec<UiVertex>,
    pub(crate) batches: Vec<UiBatch>,
}

/// What an element passes to its children.
#[derive(Clone, Copy)]
pub(super) struct Inherited {
    visible: bool,
    pub(super) alpha: f32,
    /// Pixel bounds `(min, max)`, bottom-left origin.
    clip: Option<(Vec2, Vec2)>,
}

/// What every element of one build reads: the scene, the target size in pixels and
/// the texture-size lookup.
pub(super) struct Frame<'a> {
    pub(super) world: &'a World,
    pub(super) screen: Vec2,
    tex_size: &'a dyn Fn(&str) -> Option<Vec2>,
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
    };
    let mut meshes: Vec<CanvasMesh> = Vec::new();
    let mut state: HashMap<u32, Inherited> = HashMap::new();
    for (id, rect) in layout.iter() {
        let parent = world
            .parent_id(id)
            .and_then(|p| state.get(&p).copied())
            .filter(|_| id != rect.canvas)
            .unwrap_or_else(|| root_state(world, id));
        let here = inherit(world, id, rect, parent);
        state.insert(id, here);
        if meshes.last().map(|m| m.canvas) != Some(rect.canvas) {
            frame.screen = canvas_screen(world, rect.canvas, screen);
            meshes.push(CanvasMesh {
                canvas: rect.canvas,
                ..Default::default()
            });
        }
        if let Some(mesh) = meshes.last_mut() {
            push_image(mesh, &frame, id, rect, here);
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

/// A root canvas's starting state: visible only when it and every ancestor are active.
fn root_state(world: &World, id: u32) -> Inherited {
    let mut visible = true;
    let mut cur = Some(id);
    for _ in 0..=world.len() {
        let Some(c) = cur else { break };
        visible &= world.is_active(c);
        cur = world.parent_id(c);
    }
    Inherited {
        visible,
        alpha: 1.0,
        clip: None,
    }
}

/// Fold this element's own active flag, CanvasGroup and RectMask into `parent`'s.
fn inherit(world: &World, id: u32, rect: &UiRect, parent: Inherited) -> Inherited {
    let visible = parent.visible && world.is_active(id);
    let alpha = parent.alpha * world.canvas_group(id).map_or(1.0, |g| g.alpha);
    let clip = match world.rect_mask(id) {
        Some(mask) => {
            let (lo, hi) = rect.bounds();
            let p = mask.padding;
            let lo = (lo + Vec2::new(p.x, p.y)) * rect.scale_factor;
            let hi = (hi - Vec2::new(p.z, p.w)) * rect.scale_factor;
            Some(match parent.clip {
                Some((plo, phi)) => (lo.max(plo), hi.min(phi)),
                None => (lo, hi),
            })
        }
        None => parent.clip,
    };
    Inherited {
        visible,
        alpha,
        clip,
    }
}

/// Emit `id`'s Image (if any, visible and not fully clipped) into `mesh`.
fn push_image(mesh: &mut CanvasMesh, frame: &Frame, id: u32, rect: &UiRect, state: Inherited) {
    let Some(image) = frame.world.image(id) else {
        return;
    };
    // A Selectable's ColorTint (#420) multiplies in, like the group alpha.
    let color = image.color * image.state_tint * Vec4::new(1.0, 1.0, 1.0, state.alpha);
    let Some(clip) = visible_clip(state, color.w, frame.screen) else {
        return;
    };
    let tex = image.shown_texture().and_then(frame.tex_size);
    let tris = image_triangles(&image, rect.rect.1, tex);
    let [bl, tl, _, br] = rect.corners;
    let start = mesh.vertices.len() as u32;
    mesh.vertices.extend(tris.iter().map(|&(p, uv)| UiVertex {
        pos: to_ndc(bl + (br - bl) * p.x + (tl - bl) * p.y, rect, frame.screen),
        uv: [uv.x, 1.0 - uv.y],
        color: color.to_array(),
        ..Zeroable::zeroed()
    }));
    let source = image
        .shown_texture()
        .map_or(UiSource::Solid, |t| UiSource::Texture(t.to_string()));
    close_batch(mesh, source, clip, start);
}

/// The clip a graphic draws under, or `None` when it draws nothing (hidden, fully
/// transparent, or clipped away entirely).
pub(super) fn visible_clip(state: Inherited, alpha: f32, screen: Vec2) -> Option<Option<Scissor>> {
    if !state.visible || alpha <= 0.0 {
        return None;
    }
    match state.clip {
        Some(bounds) => scissor(bounds, screen).map(Some),
        None => Some(None),
    }
}

/// A canvas point (reference units) → normalized device coordinates.
pub(super) fn to_ndc(canvas: Vec2, rect: &UiRect, screen: Vec2) -> [f32; 2] {
    (canvas * rect.scale_factor / screen * 2.0 - Vec2::ONE).to_array()
}

/// Close the vertices from `start` on into the last batch when it shares `source`
/// and `clip`, else into a new one.
pub(super) fn close_batch(
    mesh: &mut CanvasMesh,
    source: UiSource,
    clip: Option<Scissor>,
    start: u32,
) {
    let end = mesh.vertices.len() as u32;
    match mesh.batches.last_mut() {
        Some(b) if b.source == source && b.clip == clip && b.range.end == start => {
            b.range.end = end
        }
        _ if end > start => mesh.batches.push(UiBatch {
            source,
            clip,
            range: start..end,
        }),
        _ => {}
    }
}

/// Pixel bounds (bottom-left origin) → a top-left scissor clamped to the target, or
/// `None` when nothing is left.
fn scissor((lo, hi): (Vec2, Vec2), screen: Vec2) -> Option<Scissor> {
    let lo = lo.floor().clamp(Vec2::ZERO, screen);
    let hi = hi.ceil().clamp(Vec2::ZERO, screen);
    if hi.x <= lo.x || hi.y <= lo.y {
        return None;
    }
    Some(Scissor {
        x: lo.x as u32,
        y: (screen.y - hi.y) as u32,
        w: (hi.x - lo.x) as u32,
        h: (hi.y - lo.y) as u32,
    })
}

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod mesh_tests;
