pub mod geometry;
pub mod overlay;

use std::rc::Rc;

use glam::{Vec2, Vec3};

use geometry::Handle;
use rusty_components::RectTransformComponent;
use rusty_ecs::World;
use rusty_edit::authoring::ui::layout::rect_transform as rect_ops;
use rusty_ui::events::raycast_pointer;
use rusty_ui::layout::driven_by_group;
use rusty_ui::{CanvasSpace, TextureSizes, UiLayout, UiPointer, UiRect, UiView};

/// How close, in viewport points, the pointer must be to grab a handle.
const HANDLE_RADIUS: f32 = 7.0;

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayFrame {
    pub size: Vec2,
    pub pixels_per_point: f32,
    pub game: Option<(u32, u32)>,
    pub textures: Rc<TextureSizes>,
}

impl OverlayFrame {
    pub fn new(
        size: Vec2,
        pixels_per_point: f32,
        game: Option<(u32, u32)>,
        textures: &Rc<TextureSizes>,
    ) -> Self {
        let textures = Rc::clone(textures);
        Self {
            size,
            pixels_per_point,
            game,
            textures,
        }
    }

    pub fn screen(&self) -> Vec2 {
        match self.game {
            Some((w, h)) => Vec2::new(w as f32, h as f32).max(Vec2::ONE),
            None => (self.size * self.pixels_per_point).round().max(Vec2::ONE),
        }
    }

    /// The box the screen shows in, in viewport points: its top-left and extent.
    fn boxed(&self) -> (Vec2, Vec2) {
        let Some(game) = self.game else {
            return (Vec2::ZERO, self.size);
        };
        let (o, e) = rusty_core::application::letterbox((self.size.x, self.size.y), game);
        (
            Vec2::new(o.0, o.1),
            Vec2::new(e.0, e.1).max(Vec2::splat(1e-6)),
        )
    }

    /// Screen pixels per viewport point, on each axis.
    fn per_point(&self) -> Vec2 {
        match self.game {
            Some(_) => self.screen() / self.boxed().1,
            None => Vec2::splat(self.pixels_per_point),
        }
    }

    /// The UI screen pixel (bottom-left origin, y-up) under viewport point `p`
    /// (top-left origin, y-down).
    pub fn pixel(&self, p: Vec2) -> Vec2 {
        let (origin, extent) = self.boxed();
        let q = p - origin;
        Vec2::new(q.x, extent.y - q.y) * self.per_point()
    }

    pub fn to_local(&self, c: Vec2, scale: f32) -> Vec2 {
        let (origin, extent) = self.boxed();
        let pt = c * scale / self.per_point();
        origin + Vec2::new(pt.x, extent.y - pt.y)
    }

    /// A pointer move of `d` viewport points, in reference units of a canvas at
    /// `scale`.
    pub fn delta_to_canvas(&self, d: Vec2, scale: f32) -> Vec2 {
        Vec2::new(d.x, -d.y) * self.per_point() / scale.max(1e-6)
    }

    /// The overlay's layout of `world` (no camera: markers sit at their anchors).
    pub fn layout(&self, world: &World) -> UiLayout {
        let view = UiView::screen(self.screen(), Rc::clone(&self.textures));
        UiLayout::compute_in(world, &view)
    }
}

pub struct Target {
    pub rect: UiRect,
    pub parent: UiRect,
    pub rt: RectTransformComponent,
    /// False for a root canvas, a world-anchored marker or an element a layout group
    /// places: drawn read-only, as Unity greys them.
    pub editable: bool,
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectDrag {
    pub id: u32,
    pub handle: Handle,
}

pub fn begin(world: &World, frame: &OverlayFrame, id: u32, p: Vec2) -> Option<RectDrag> {
    let t = target(world, &frame.layout(world), id).filter(|t| t.editable)?;
    let to = |c: Vec2| frame.to_local(c, t.rect.scale_factor);
    let handle = geometry::hit(&t.rect, t.rt.pivot, to, p, HANDLE_RADIUS)?;
    Some(RectDrag { id, handle })
}

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
