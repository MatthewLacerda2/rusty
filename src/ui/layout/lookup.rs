//! src/ui/layout/lookup.rs — one element's rect on demand, without the whole pass (#417).
//!
//! `UI.GetRect`, `Text.GetPreferredSize` and `Debug.Snapshot` ask for a single rect
//! from the live scene (edit mode included): walk only its ancestor chain with the
//! same maths [`UiLayout::compute_in`](super::UiLayout::compute_in) runs.

use glam::Vec2;

use super::{child_node, group, root_canvas_of, root_node, UiRect};
use crate::ecs::World;
use crate::ui::UiView;

/// The rect of one entity, computed on demand by walking only its ancestor chain —
/// identical to what [`UiLayout::compute`](super::UiLayout::compute) yields for it. `None` when the entity is
/// not under a canvas, or a link of the chain lacks a `RectTransform`.
pub fn rect_of(world: &World, id: u32, screen: Vec2) -> Option<UiRect> {
    rect_in(world, id, &UiView::screen(screen))
}

/// [`rect_of`] against `view` (markers placed, world canvases at their reference
/// resolution).
pub fn rect_in(world: &World, id: u32, view: &UiView) -> Option<UiRect> {
    let root = root_canvas_of(world, id)?;
    let mut chain = Vec::new();
    let mut cur = id;
    while cur != root {
        chain.push(cur);
        cur = world.parent_id(cur)?;
    }
    let (mut node, _) = root_node(world, root, view)?;
    let mut parent = root;
    for &link in chain.iter().rev() {
        let placed = group::placements(world, parent, node.rect.rect)
            .into_iter()
            .find_map(|(c, r)| (c == link).then_some(r));
        node = child_node(world, view, link, &node, placed)?;
        parent = link;
    }
    Some(node.rect)
}
