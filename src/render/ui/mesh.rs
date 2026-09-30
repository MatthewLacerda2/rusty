//! src/render/ui/mesh.rs — the laid-out UI → one vertex list + batches per canvas (#418).
//!
//! GPU-free: [`build_canvas_meshes`] walks the layout in draw order (canvas
//! `sort_order`, then hierarchy pre-order) and emits every visible `Image` as
//! triangles in normalized device coordinates, tagged with what it samples and how
//! it is clipped. Consecutive graphics sharing a texture and a clip merge into one
//! [`UiBatch`]; a change of either starts a new one — so a HUD of solid bars is one
//! draw call per canvas however many bars it has.
//!
//! Inherited state rides down the walk (parents precede children in the layout):
//! **visibility** (the entity and every ancestor active), **alpha** (the product of
//! every `CanvasGroup` on the chain) and **clip** (every `RectMask` on the chain,
//! intersected, as a pixel scissor rect).

use std::collections::HashMap;

use glam::{Vec2, Vec4};

use super::geometry::image_triangles;
use crate::ecs::World;
use crate::ui::{UiLayout, UiRect};

/// One UI vertex: NDC position, texture coordinate (v down, as the GPU samples) and
/// the straight-alpha display-space tint (group alpha folded in).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct UiVertex {
    pub(crate) pos: [f32; 2],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
}

/// A scissor rect in target pixels, top-left origin (wgpu's convention).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scissor {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

/// A run of vertices drawn with one texture and one clip.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiBatch {
    /// The texture path sampled, or `None` for a solid colour.
    pub(crate) texture: Option<String>,
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
struct Inherited {
    visible: bool,
    alpha: f32,
    /// Pixel bounds `(min, max)`, bottom-left origin.
    clip: Option<(Vec2, Vec2)>,
}

/// What every element of one build reads: the scene, the target size in pixels and
/// the texture-size lookup.
struct Frame<'a> {
    world: &'a World,
    screen: Vec2,
    tex_size: &'a dyn Fn(&str) -> Option<Vec2>,
}

/// Build every canvas's mesh on a `screen`-pixel target. `tex_size` reports a
/// texture's size in texels (`None` when not loaded).
pub(crate) fn build_canvas_meshes(
    world: &World,
    layout: &UiLayout,
    screen: Vec2,
    tex_size: &dyn Fn(&str) -> Option<Vec2>,
) -> Vec<CanvasMesh> {
    let frame = Frame {
        world,
        screen: screen.max(Vec2::ONE),
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
            meshes.push(CanvasMesh {
                canvas: rect.canvas,
                ..Default::default()
            });
        }
        if let Some(mesh) = meshes.last_mut() {
            push_image(mesh, &frame, id, rect, here);
        }
    }
    meshes.retain(|m| !m.batches.is_empty());
    meshes
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
    let screen = frame.screen;
    let color = image.color * Vec4::new(1.0, 1.0, 1.0, state.alpha);
    if !state.visible || color.w <= 0.0 {
        return;
    }
    let clip = match state.clip {
        Some(bounds) => match scissor(bounds, screen) {
            Some(s) => Some(s),
            None => return, // clipped away entirely
        },
        None => None,
    };
    let tex = image.texture.as_deref().and_then(frame.tex_size);
    let tris = image_triangles(&image, rect.rect.1, tex);
    let [bl, tl, _, br] = rect.corners;
    let start = mesh.vertices.len() as u32;
    mesh.vertices.extend(tris.iter().map(|&(p, uv)| {
        let canvas = bl + (br - bl) * p.x + (tl - bl) * p.y;
        let ndc = canvas * rect.scale_factor / screen * 2.0 - Vec2::ONE;
        UiVertex {
            pos: ndc.to_array(),
            uv: [uv.x, 1.0 - uv.y],
            color: color.to_array(),
        }
    }));
    let end = mesh.vertices.len() as u32;
    let texture = image.texture.clone();
    match mesh.batches.last_mut() {
        Some(b) if b.texture == texture && b.clip == clip && b.range.end == start => {
            b.range.end = end
        }
        _ if end > start => mesh.batches.push(UiBatch {
            texture,
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
