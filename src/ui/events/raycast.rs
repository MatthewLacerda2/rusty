//! src/ui/events/raycast.rs — which UI graphic is under a point (#420).
//!
//! Unity's `GraphicRaycaster`, CPU-only so a headless run hits exactly what a
//! window would. A [`UiPointer`] is a point in **UI screen pixels** (bottom-left
//! origin, y-up — the frame `UI.GetRect`'s `screen` box is in) and the camera ray
//! through it (#429). Screen canvases are tried first — they draw over the world —
//! top-most first: the layout is in draw order (canvas `sort_order`, then hierarchy
//! pre-order), so it is walked backwards. Then the world canvases (`WorldSpace`,
//! `ScreenSpaceCamera`) the ray crosses, nearest plane first, each hit-tested at the
//! crossing point in its own reference units; `ray_limit` (the nearest wall, when
//! the caller knows it) stops the ray. A graphic can be hit when:
//!
//! - it is an `Image` or `Text` with `raycast_target`;
//! - it and every ancestor are active;
//! - no `CanvasGroup` on it or above it has `blocks_raycasts = false`;
//! - the point is inside every `RectMask` above it (and its own), padding applied —
//!   the same axis-aligned screen clip drawing uses;
//! - the point is inside its final quad, rotation and scale included.
//!
//! A fully transparent graphic still blocks (Unity's default alpha threshold of 0).

use glam::Vec2;

use super::tree::{ancestors_or_self, is_visible, raycasts_blocked};
use crate::ecs::World;
use crate::ui::space::ray_to_canvas;
use crate::ui::{CanvasSpace, UiLayout, UiPointer, UiRect};

/// The top-most raycast target under `point` (UI screen pixels) on the screen
/// canvases, if any.
pub fn raycast(world: &World, layout: &UiLayout, point: Vec2) -> Option<u32> {
    raycast_pointer(world, layout, &UiPointer::at(point))
}

/// The raycast target `pointer` hits: the screen canvases first, then the world
/// canvases along its ray, nearest first (see the module docs).
pub fn raycast_pointer(world: &World, layout: &UiLayout, pointer: &UiPointer) -> Option<u32> {
    let rects: Vec<(u32, &UiRect)> = layout.iter().collect();
    let hit_in = |canvas: u32, p: Vec2| {
        rects
            .iter()
            .rev()
            .filter(|(_, r)| r.canvas == canvas)
            .find(|&&(id, rect)| is_hit(world, layout, id, rect, p))
            .map(|&(id, _)| id)
    };
    let canvases: Vec<(u32, CanvasSpace)> = layout.canvases().collect();
    if let Some(px) = pointer.screen {
        let screen = canvases
            .iter()
            .rev()
            .filter(|(_, s)| *s == CanvasSpace::Screen);
        for &(c, _) in screen {
            let scale = layout.get(c).map_or(1.0, |r| r.scale_factor);
            if let Some(hit) = hit_in(c, px / scale) {
                return Some(hit);
            }
        }
    }
    let ray = pointer.ray?;
    let mut planes: Vec<(f32, u32, Vec2)> = canvases
        .iter()
        .filter_map(|&(c, space)| match space {
            CanvasSpace::World(m) => ray_to_canvas(m, ray).map(|(p, d)| (d, c, p)),
            CanvasSpace::Screen => None,
        })
        .filter(|&(d, _, _)| d <= pointer.ray_limit)
        .collect();
    planes.sort_by(|a, b| a.0.total_cmp(&b.0));
    planes.into_iter().find_map(|(_, c, p)| hit_in(c, p))
}

/// Whether `id` (laid out as `rect`) takes a hit at `point` (its canvas's
/// reference units).
fn is_hit(world: &World, layout: &UiLayout, id: u32, rect: &UiRect, point: Vec2) -> bool {
    is_raycast_target(world, id)
        && in_quad(rect, point)
        && is_visible(world, id)
        && !raycasts_blocked(world, id)
        && inside_masks(world, layout, id, point)
}

/// An `Image` or `Text` with `raycast_target` on.
fn is_raycast_target(world: &World, id: u32) -> bool {
    world.image(id).is_some_and(|i| i.raycast_target)
        || world.text(id).is_some_and(|t| t.raycast_target)
}

/// Whether `p` (reference units) is inside the rect's final quad. The corners wind
/// consistently (bottom-left, top-left, top-right, bottom-right), mirrored or not, so
/// the point is inside when every edge sees it on the same side. A degenerate
/// (zero-area) quad is never hit.
fn in_quad(rect: &UiRect, p: Vec2) -> bool {
    let c = rect.corners;
    let sides = (0..4).map(|i| {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        (b - a).perp_dot(p - a)
    });
    let (mut pos, mut neg) = (false, false);
    for s in sides {
        pos |= s > 0.0;
        neg |= s < 0.0;
    }
    let area = (c[2] - c[0]).perp_dot(c[3] - c[1]);
    area != 0.0 && !(pos && neg)
}

/// Whether `point` (reference units) is inside every `RectMask` on `id`'s chain.
fn inside_masks(world: &World, layout: &UiLayout, id: u32, point: Vec2) -> bool {
    ancestors_or_self(world, id).into_iter().all(|c| {
        let (Some(mask), Some(rect)) = (world.rect_mask(c), layout.get(c)) else {
            return true;
        };
        let (lo, hi) = rect.bounds();
        let p = mask.padding;
        let lo = lo + Vec2::new(p.x, p.y);
        let hi = hi - Vec2::new(p.z, p.w);
        point.cmpge(lo).all() && point.cmple(hi).all()
    })
}

#[cfg(test)]
#[path = "raycast_tests.rs"]
mod raycast_tests;
