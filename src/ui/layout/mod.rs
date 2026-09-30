//! src/ui/layout.rs — the UI layout pass: canvases + rect transforms → rectangles (#417).
//!
//! Pure and GPU-free: [`UiLayout::compute`] reads the ECS world and the screen size
//! and returns every UI element's rect, so the headless harness lays out exactly as
//! the window does and a bot can reason about (and later hit-test) the UI.
//!
//! **Coordinates.** Everything is in the owning canvas's *reference units*, y-up,
//! origin at the canvas's bottom-left — Unity's convention. Screen pixels are the
//! same frame multiplied by the canvas scale factor (bottom-left origin, y-up).
//!
//! **The walk.** Each *root* canvas (a `Canvas` with no `Canvas` ancestor) spans the
//! whole screen: its rect is `(0, 0)`–`canvas.size(screen)` and its Transform is
//! ignored. Below it, every descendant with a `RectTransform` is laid out inside its
//! parent's rect ([`RectTransformComponent::layout_in`]), then its Transform's
//! rotation and scale are applied around its pivot and composed with the parent's —
//! so a rotated panel carries its children with it. A descendant without a
//! `RectTransform` ends the UI subtree (it and its children get no rect). A nested
//! `Canvas` below a root is laid out like any other rect. Inactive entities are
//! still laid out; drawing and hit-testing filter on `active`.
//!
//! **Layout groups (#421).** A `LayoutGroup` on an element overrides where its
//! layout children go: the walk places each at the group's computed rect instead
//! of its anchors (`group`, `grid`), from the min / preferred / flexible sizes
//! its content reports (`sizes`). Any other element with a `LayoutElement`
//! content fitter is resized to its content around its pivot (`fit`). The scene's
//! RectTransforms are never rewritten — the pass stays a pure function, recomputed
//! whole each tick.
//!
//! **Order.** [`UiLayout::iter`] yields rects in draw order: root canvases by
//! `sort_order` (ties keep scene insertion order), then each canvas's hierarchy in
//! pre-order (a later sibling draws on top), as in Unity.

use std::collections::BTreeMap;

use glam::{Mat4, Vec2, Vec3};

use crate::components::{RectTransformComponent, TransformComponent};
use crate::ecs::World;

mod fit;
#[cfg(test)]
mod fixture;
mod grid;
mod group;
mod native;
mod sizes;

pub use sizes::{element_sizes, Sizes};

/// One laid-out UI element (the canvas root included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiRect {
    /// The root canvas entity this element lays out under.
    pub canvas: u32,
    /// Screen pixels per reference unit for that canvas.
    pub scale_factor: f32,
    /// The element's own rect, `(min, size)`, before rotation and scale — in the
    /// same frame as its parent's rect (Unity's layout rect).
    pub rect: (Vec2, Vec2),
    /// The final quad in canvas reference units after the whole rotation / scale
    /// chain: bottom-left, top-left, top-right, bottom-right (Unity's
    /// `GetWorldCorners` order).
    pub corners: [Vec2; 4],
}

impl UiRect {
    /// Axis-aligned bounds of the final quad in reference units, `(min, max)`.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut lo = self.corners[0];
        let mut hi = self.corners[0];
        for c in &self.corners[1..] {
            lo = lo.min(*c);
            hi = hi.max(*c);
        }
        (lo, hi)
    }

    /// Axis-aligned bounds of the final quad in screen pixels, `(min, max)`.
    pub fn screen_bounds(&self) -> (Vec2, Vec2) {
        let (lo, hi) = self.bounds();
        (lo * self.scale_factor, hi * self.scale_factor)
    }
}

/// Every UI element's computed rect, in draw order (a resource; see the module docs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayout {
    rects: Vec<(u32, UiRect)>,
    index: BTreeMap<u32, usize>,
}

impl UiLayout {
    /// Lay out every root canvas in `world` on a `screen`-pixel screen.
    pub fn compute(world: &World, screen: Vec2) -> Self {
        let mut roots: Vec<(i32, u32)> = world
            .ids_with_canvas()
            .into_iter()
            .filter(|&id| root_canvas_of(world, id) == Some(id))
            .filter_map(|id| Some((world.canvas(id)?.sort_order, id)))
            .collect();
        // Stable: equal sort orders keep insertion order.
        roots.sort_by_key(|&(order, _)| order);
        let mut layout = Self::default();
        for (_, root) in roots {
            if let Some(node) = root_node(world, root, screen) {
                layout.walk(world, root, node);
            }
        }
        layout
    }

    /// The rect of `id`, when it was laid out.
    pub fn get(&self, id: u32) -> Option<&UiRect> {
        self.index.get(&id).map(|&i| &self.rects[i].1)
    }

    /// Every laid-out element, in draw order (back to front).
    pub fn iter(&self) -> impl Iterator<Item = (u32, &UiRect)> {
        self.rects.iter().map(|(id, r)| (*id, r))
    }

    /// Number of laid-out elements.
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    /// Whether nothing was laid out (no canvas in the scene).
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Record `id`'s node, then its rect-carrying children in hierarchy order.
    fn walk(&mut self, world: &World, id: u32, node: Node) {
        self.index.insert(id, self.rects.len());
        self.rects.push((id, node.rect));
        let placed: BTreeMap<u32, _> = group::placements(world, id, node.rect.rect)
            .into_iter()
            .collect();
        for child in world.children(id) {
            if self.index.contains_key(&child) {
                continue; // a malformed hierarchy never loops the walk
            }
            if let Some(child_node) = child_node(world, child, &node, placed.get(&child).copied()) {
                self.walk(world, child, child_node);
            }
        }
    }
}

/// The rect of one entity, computed on demand by walking only its ancestor chain —
/// identical to what [`UiLayout::compute`] yields for it. `None` when the entity is
/// not under a canvas, or a link of the chain lacks a `RectTransform`.
pub fn rect_of(world: &World, id: u32, screen: Vec2) -> Option<UiRect> {
    let root = root_canvas_of(world, id)?;
    let mut chain = Vec::new();
    let mut cur = id;
    while cur != root {
        chain.push(cur);
        cur = world.parent_id(cur)?;
    }
    let mut node = root_node(world, root, screen)?;
    let mut parent = root;
    for &link in chain.iter().rev() {
        let placed = group::placements(world, parent, node.rect.rect)
            .into_iter()
            .find_map(|(c, r)| (c == link).then_some(r));
        node = child_node(world, link, &node, placed)?;
        parent = link;
    }
    Some(node.rect)
}

/// A laid-out element plus the matrix taking its rect's frame to canvas space.
struct Node {
    rect: UiRect,
    /// Maps this element's layout frame (where `rect.rect` lives, and where its
    /// children's rects are laid out) into canvas reference units.
    to_canvas: Mat4,
}

/// The topmost `Canvas` on `id`'s ancestor chain (`id` itself included) — the root
/// its UI lays out under. Bounded by the entity count, so a malformed parent cycle
/// cannot hang it.
fn root_canvas_of(world: &World, id: u32) -> Option<u32> {
    let mut root = None;
    let mut cur = Some(id);
    for _ in 0..=world.len() {
        let Some(c) = cur else { break };
        if world.has_canvas(c) {
            root = Some(c);
        }
        cur = world.parent_id(c);
    }
    root
}

/// A root canvas spans the screen in reference units; its Transform is ignored.
fn root_node(world: &World, id: u32, screen: Vec2) -> Option<Node> {
    let canvas = world.canvas(id)?;
    let size = canvas.size(screen);
    let rect = UiRect {
        canvas: id,
        scale_factor: canvas.scale_factor(screen),
        rect: (Vec2::ZERO, size),
        corners: quad(Mat4::IDENTITY, Vec2::ZERO, size),
    };
    Some(Node {
        rect,
        to_canvas: Mat4::IDENTITY,
    })
}

/// Lay `id` out inside `parent` — at `placed` when the parent's layout group
/// arranges it, else by its own RectTransform and content fitter — composing its
/// pivot-centred rotation and scale.
fn child_node(world: &World, id: u32, parent: &Node, placed: Option<(Vec2, Vec2)>) -> Option<Node> {
    let rt = world.rect_transform(id)?;
    let (min, size) = placed.unwrap_or_else(|| {
        let own = rt.layout_in(parent.rect.rect.0, parent.rect.rect.1);
        fit::fit(world, id, &rt, own)
    });
    let transform = world.transform(id)?;
    let to_canvas = parent.to_canvas * pivot_matrix(&rt, &transform, min, size);
    let rect = UiRect {
        canvas: parent.rect.canvas,
        scale_factor: parent.rect.scale_factor,
        rect: (min, size),
        corners: quad(to_canvas, min, size),
    };
    Some(Node { rect, to_canvas })
}

/// Rotation and scale around the pivot point: `T(p) · R · S · T(-p)`. The
/// Transform's position is deliberately unused — the rect places the element.
fn pivot_matrix(
    rt: &RectTransformComponent,
    t: &TransformComponent,
    min: Vec2,
    size: Vec2,
) -> Mat4 {
    let p = (min + size * rt.pivot).extend(0.0);
    Mat4::from_translation(p)
        * Mat4::from_scale_rotation_translation(t.scale, t.rotation, Vec3::ZERO)
        * Mat4::from_translation(-p)
}

/// The rect's four corners through `m`, projected orthographically onto the canvas
/// plane: bottom-left, top-left, top-right, bottom-right.
fn quad(m: Mat4, min: Vec2, size: Vec2) -> [Vec2; 4] {
    let max = min + size;
    [
        Vec2::new(min.x, min.y),
        Vec2::new(min.x, max.y),
        Vec2::new(max.x, max.y),
        Vec2::new(max.x, min.y),
    ]
    .map(|c| m.transform_point3(c.extend(0.0)).truncate())
}

#[cfg(test)]
mod tests;
