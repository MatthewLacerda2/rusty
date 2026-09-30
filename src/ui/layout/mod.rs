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
//!
//! **Views (#429).** [`UiLayout::compute_in`] lays out against a [`UiView`] — the
//! screen and the camera. A `WorldSpace` root spans its reference resolution
//! whatever the screen; every root's [`CanvasSpace`] (screen, or a world plane) is
//! recorded beside its rects. With a camera, an element carrying a `WorldAnchor`
//! is a marker placed by projection (`anchor`); a hidden marker is left out with
//! its subtree. [`UiLayout::compute`] is the camera-less view.

use std::collections::BTreeMap;

use glam::{Mat4, Vec2, Vec3};

use crate::components::{RectTransformComponent, TransformComponent};
use crate::ecs::World;
use crate::ui::space::{canvas_space, CanvasSpace, UiView};

mod anchor;
mod fit;
#[cfg(test)]
mod fixture;
mod grid;
mod group;
mod lookup;
mod native;
mod rect;
mod sizes;

pub use lookup::{rect_in, rect_of};
pub use rect::UiRect;
pub use sizes::{element_sizes, Sizes};

/// Every UI element's computed rect, in draw order (a resource; see the module docs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayout {
    rects: Vec<(u32, UiRect)>,
    index: BTreeMap<u32, usize>,
    spaces: BTreeMap<u32, CanvasSpace>,
}

impl UiLayout {
    /// Lay out every root canvas in `world` on a `screen`-pixel screen, with no
    /// camera (no world planes, no markers).
    pub fn compute(world: &World, screen: Vec2) -> Self {
        Self::compute_in(world, &UiView::screen(screen))
    }

    /// Lay out every root canvas in `world` against `view`.
    pub fn compute_in(world: &World, view: &UiView) -> Self {
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
            if let Some((node, space)) = root_node(world, root, view) {
                layout.spaces.insert(root, space);
                layout.walk(world, view, root, node);
            }
        }
        layout
    }

    /// Where root canvas `canvas`'s reference units end up (the screen for an
    /// unknown id).
    pub fn space(&self, canvas: u32) -> CanvasSpace {
        self.spaces.get(&canvas).copied().unwrap_or_default()
    }

    /// Every laid-out root canvas and its space, in draw order.
    pub fn canvases(&self) -> impl Iterator<Item = (u32, CanvasSpace)> + '_ {
        self.rects
            .iter()
            .filter(|(id, r)| *id == r.canvas)
            .map(|(id, _)| (*id, self.space(*id)))
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
    fn walk(&mut self, world: &World, view: &UiView, id: u32, node: Node) {
        self.index.insert(id, self.rects.len());
        self.rects.push((id, node.rect));
        let placed: BTreeMap<u32, _> = group::placements(world, id, node.rect.rect)
            .into_iter()
            .collect();
        for child in world.children(id) {
            if self.index.contains_key(&child) {
                continue; // a malformed hierarchy never loops the walk
            }
            let placed = placed.get(&child).copied();
            if let Some(child_node) = child_node(world, view, child, &node, placed) {
                self.walk(world, view, child, child_node);
            }
        }
    }
}

/// A laid-out element plus the matrix taking its rect's frame to canvas space.
struct Node {
    rect: UiRect,
    /// Maps this element's layout frame (where `rect.rect` lives, and where its
    /// children's rects are laid out) into canvas reference units.
    to_canvas: Mat4,
    /// The root canvas's size, when markers are placed on it (a screen-space root
    /// seen through a camera); `None` otherwise.
    markers: Option<Vec2>,
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

/// A root canvas spans the screen (or, `WorldSpace`, its reference resolution) in
/// reference units; its Transform only places a world canvas's plane.
fn root_node(world: &World, id: u32, view: &UiView) -> Option<(Node, CanvasSpace)> {
    let canvas = world.canvas(id)?;
    let size = canvas.size(view.screen);
    let rect = UiRect {
        canvas: id,
        scale_factor: canvas.scale_factor(view.screen),
        rect: (Vec2::ZERO, size),
        corners: quad(Mat4::IDENTITY, Vec2::ZERO, size),
    };
    let markers = (view.camera.is_some() && !canvas.is_world_space()).then_some(size);
    let node = Node {
        rect,
        to_canvas: Mat4::IDENTITY,
        markers,
    };
    Some((node, canvas_space(world, id, &canvas, view)))
}

/// Lay `id` out inside `parent` — pinned by its world anchor when it is a marker,
/// at `placed` when the parent's layout group arranges it, else by its own
/// RectTransform and content fitter — composing its pivot-centred rotation and
/// scale. `None` when it is not a UI element, or is a hidden marker.
fn child_node(
    world: &World,
    view: &UiView,
    id: u32,
    parent: &Node,
    placed: Option<(Vec2, Vec2)>,
) -> Option<Node> {
    let rt = world.rect_transform(id)?;
    let own = || {
        let own = rt.layout_in(parent.rect.rect.0, parent.rect.rect.1);
        fit::fit(world, id, &rt, own)
    };
    let marker = rt
        .world_anchor
        .as_ref()
        .zip(parent.markers)
        .and_then(|(a, size)| anchor::place(world, a, view, size, parent.rect.scale_factor));
    let ((min, size), spin) = match marker {
        Some(anchor::Placement::Hidden) => return None,
        Some(anchor::Placement::At { pivot, angle }) => {
            let (_, size) = own();
            let local = parent
                .to_canvas
                .inverse()
                .transform_point3(pivot.extend(0.0));
            ((local.truncate() - size * rt.pivot, size), angle)
        }
        None => (placed.unwrap_or_else(own), 0.0),
    };
    let transform = world.transform(id)?;
    let spin = Mat4::from_rotation_z(spin);
    let to_canvas = parent.to_canvas * pivot_matrix(&rt, &transform, min, size, spin);
    let rect = UiRect {
        canvas: parent.rect.canvas,
        scale_factor: parent.rect.scale_factor,
        rect: (min, size),
        corners: quad(to_canvas, min, size),
    };
    Some(Node {
        rect,
        to_canvas,
        markers: parent.markers,
    })
}

/// Rotation and scale around the pivot point: `T(p) · spin · R · S · T(-p)`, where
/// `spin` is a marker's turn toward its target. The Transform's position is
/// deliberately unused — the rect places the element.
fn pivot_matrix(
    rt: &RectTransformComponent,
    t: &TransformComponent,
    min: Vec2,
    size: Vec2,
    spin: Mat4,
) -> Mat4 {
    let p = (min + size * rt.pivot).extend(0.0);
    Mat4::from_translation(p)
        * spin
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
