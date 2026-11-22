//! crates/ui/src/layout/lookup.rs — one element's rect on demand, without the whole pass.

use super::sizes::layout_children;
use super::{child_node, group, root_canvas_of, root_node, UiRect};
use crate::UiView;
use rusty_ecs::World;

/// The rect of one entity against `view`, computed on demand by walking only its
/// ancestor chain — identical to what [`UiLayout::compute_in`](super::UiLayout::compute_in)
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
        let placed = group::placements(world, &view.textures, parent, node.rect.rect)
            .into_iter()
            .find_map(|(c, r)| (c == link).then_some(r));
        node = child_node(world, view, link, &node, placed)?;
        parent = link;
    }
    Some(node.rect)
}

/// Whether the layout group on `id`'s parent places it — its anchors, position and
/// size are then driven, and the editor shows them read-only, as Unity does.
pub fn driven_by_group(world: &World, id: u32) -> bool {
    world
        .parent_id(id)
        .is_some_and(|p| world.layout_group(p).is_some() && layout_children(world, p).contains(&id))
}
