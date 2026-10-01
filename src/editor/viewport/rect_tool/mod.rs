//! src/editor/viewport/rect_tool/ — authoring UI in the Scene view (#423).
//!
//! Unity's Rect Tool over the Scene tab's **UI overlay** (its toggle draws the
//! screen-space canvases over the scene): drag inside the selected element to move
//! it (`anchored_position`), its edges and corners to resize it (`size_delta`, the
//! opposite side held), its pivot disc to move the pivot without moving the rect.
//! Anchors show as Unity's four triangles. Every write goes through the
//! `scene::authoring::rect_transform` setters the inspector card and the Lua
//! `RectTransform.*` verbs use, so nothing here is an editor-only capability.
//!
//! **What it edits.** Elements on screen canvases (`ScreenSpaceOverlay`, and
//! `ScreenSpaceCamera`, whose plane fills the view the overlay shows). An element a
//! layout group places, a world-anchored marker and a root canvas are drawn
//! read-only, as Unity greys them. World-space canvases are click-selectable but
//! not dragged here (their numbers are on the card).
//!
//! **Click-select** runs `UI.Raycast`'s hit-test ([`raycast_pointer`]): the overlay
//! canvases first, then the world canvases along the Scene camera's ray, stopped
//! at the nearest mesh. So only raycast targets (an `Image` / `Text` with
//! `raycast_target`) are picked by clicking; anything else is picked in the
//! hierarchy.
//!
//! This module is egui-free maths and state; `overlay` paints it.

pub mod geometry;
pub mod overlay;

use glam::{Vec2, Vec3};

use crate::components::RectTransformComponent;
use crate::ecs::World;
use crate::scene::authoring::rect_transform as rect_ops;
use crate::ui::events::raycast_pointer;
use crate::ui::layout::driven_by_group;
use crate::ui::{CanvasSpace, UiLayout, UiPointer, UiRect};
use geometry::Handle;

/// How close (viewport points) the pointer must be to grab a handle.
pub const HANDLE_RADIUS: f32 = 7.0;

/// The Scene viewport image as the overlay sees it: its size in points and the
/// pixels per point the offscreen target was sized with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayFrame {
    pub size: Vec2,
    pub pixels_per_point: f32,
}

impl OverlayFrame {
    /// The screen the overlay lays out on: the viewport target's pixel size.
    pub fn screen(&self) -> Vec2 {
        (self.size * self.pixels_per_point).round().max(Vec2::ONE)
    }

    /// The UI screen pixel (bottom-left origin, y-up) under viewport point `p`
    /// (top-left origin, y-down).
    pub fn pixel(&self, p: Vec2) -> Vec2 {
        Vec2::new(p.x, self.size.y - p.y) * self.pixels_per_point
    }

    /// The viewport point showing canvas point `c` of a canvas at `scale` pixels
    /// per reference unit.
    pub fn to_local(&self, c: Vec2, scale: f32) -> Vec2 {
        let px = c * scale / self.pixels_per_point;
        Vec2::new(px.x, self.size.y - px.y)
    }

    /// A pointer move of `d` viewport points, in reference units of a canvas at
    /// `scale`.
    pub fn delta_to_canvas(&self, d: Vec2, scale: f32) -> Vec2 {
        Vec2::new(d.x, -d.y) * self.pixels_per_point / scale.max(1e-6)
    }

    /// The overlay's layout of `world` (no camera: markers sit at their anchors).
    pub fn layout(&self, world: &World) -> UiLayout {
        UiLayout::compute(world, self.screen())
    }
}

/// The selected element as the rect tool sees it.
pub struct Target {
    pub rect: UiRect,
    pub parent: UiRect,
    pub rt: RectTransformComponent,
    /// Whether the tool may drag it (see the module docs).
    pub editable: bool,
}

/// `id` on a screen canvas of `layout`, or `None` (not UI, or a world canvas).
pub fn target(world: &World, layout: &UiLayout, id: u32) -> Option<Target> {
    let rect = *layout.get(id)?;
    if layout.space(rect.canvas) != CanvasSpace::Screen {
        return None;
    }
    let rt = world
        .rect_transform(id)
        .map(|r| r.clone())
        .unwrap_or_default();
    let parent = world.parent_id(id).and_then(|p| layout.get(p)).copied();
    let root = id == rect.canvas;
    let editable = !root && rt.world_anchor.is_none() && !driven_by_group(world, id);
    Some(Target {
        parent: parent.unwrap_or(rect),
        rect,
        rt,
        editable,
    })
}

/// A live rect-tool drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectDrag {
    pub id: u32,
    pub handle: Handle,
}

/// Grab a handle of the selected element `id` at viewport point `p`; `None` when
/// it is read-only, not on the overlay, or `p` misses it.
pub fn begin(world: &World, frame: &OverlayFrame, id: u32, p: Vec2) -> Option<RectDrag> {
    let t = target(world, &frame.layout(world), id).filter(|t| t.editable)?;
    let to = |c: Vec2| frame.to_local(c, t.rect.scale_factor);
    let handle = geometry::hit(&t.rect, t.rt.pivot, to, p, HANDLE_RADIUS)?;
    Some(RectDrag { id, handle })
}

/// Apply a pointer move of `d` viewport points to the drag through the shared
/// setters. Returns whether anything was written.
pub fn apply(world: &mut World, frame: &OverlayFrame, drag: RectDrag, d: Vec2) -> bool {
    let Some(t) = target(world, &frame.layout(world), drag.id) else {
        return false;
    };
    let delta = frame.delta_to_canvas(d, t.rect.scale_factor);
    let Some(edit) = geometry::drag(&t.rt, drag.handle, delta, &t.rect, &t.parent) else {
        return false;
    };
    let Some(mut r) = world.rect_transform_mut(drag.id) else {
        return false;
    };
    rect_ops::set_anchored_position(&mut r, edit.anchored_position);
    rect_ops::set_size_delta(&mut r, edit.size_delta);
    rect_ops::set_pivot(&mut r, edit.pivot);
    delta != Vec2::ZERO
}

/// The UI element a click at viewport point `p` picks: the overlay canvases when
/// `overlay` is on, then the world canvases along `ray` (origin, unit direction)
/// up to `limit` metres — `UI.Raycast`'s own hit-test.
pub fn pick(
    world: &World,
    frame: &OverlayFrame,
    p: Vec2,
    overlay: bool,
    ray: (Vec3, Vec3),
    limit: f32,
) -> Option<u32> {
    let pointer = UiPointer {
        screen: overlay.then(|| frame.pixel(p)),
        ray: Some(ray),
        ray_limit: limit,
    };
    raycast_pointer(world, &frame.layout(world), &pointer)
}

#[cfg(test)]
mod tests;
