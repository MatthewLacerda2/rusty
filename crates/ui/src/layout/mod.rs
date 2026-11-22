use std::collections::BTreeMap;

use glam::{Mat4, Vec2, Vec3};

use crate::space::{canvas_space, CanvasSpace, UiView};
use rusty_components::{RectTransformComponent, TransformComponent};
use rusty_ecs::World;

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

pub use lookup::{driven_by_group, rect_in};
pub use native::TextureSizes;
pub use rect::UiRect;
pub use sizes::Sizes;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayout {
    rects: Vec<(u32, UiRect)>,
    index: BTreeMap<u32, usize>,
    spaces: BTreeMap<u32, CanvasSpace>,
}

impl UiLayout {
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

    pub fn respace(&mut self, world: &World, view: &UiView) {
        for (&root, space) in &mut self.spaces {
            *space = crate::space::space_of(world, root, view);
        }
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

    pub fn has_world_canvas(&self) -> bool {
        self.spaces
            .values()
            .any(|s| matches!(s, CanvasSpace::World(_)))
    }

    pub fn get(&self, id: u32) -> Option<&UiRect> {
        self.index.get(&id).map(|&i| &self.rects[i].1)
    }

    /// Every laid-out element, in draw order (back to front).
    pub fn iter(&self) -> impl Iterator<Item = (u32, &UiRect)> {
        self.rects.iter().map(|(id, r)| (*id, r))
    }

    /// Record `id`'s node, then its rect-carrying children in hierarchy order.
    fn walk(&mut self, world: &World, view: &UiView, id: u32, node: Node) {
        self.index.insert(id, self.rects.len());
        self.rects.push((id, node.rect));
        let placed: BTreeMap<u32, _> = group::placements(world, &view.textures, id, node.rect.rect)
            .into_iter()
            .collect();
        for child in world.children(id) {
            if self.index.contains_key(&child) {
                // A malformed parent cycle never loops the walk.
                continue;
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
    to_canvas: Mat4,
    markers: Option<Vec2>,
}

fn root_canvas_of(world: &World, id: u32) -> Option<u32> {
    world
        .self_and_ancestors(id)
        .filter(|&c| world.has_canvas(c))
        .last()
}

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
        fit::fit(world, &view.textures, id, &rt, own)
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

/// The Transform's position is unused: the rect places the element.
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
