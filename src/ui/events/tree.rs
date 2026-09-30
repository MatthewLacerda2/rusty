//! src/ui/events/tree.rs — hierarchy questions the event system asks (#420).
//!
//! Every rule that depends on an entity's ancestors lives here, once: whether it is
//! visible (it and every ancestor `active`), whether a `CanvasGroup` above it stops
//! the pointer or disables its selectables, and the ancestor-or-self walk event
//! bubbling follows. Each walk is bounded by the entity count, so a malformed parent
//! cycle cannot hang the sim.

use crate::ecs::World;

/// `id`, then its parent, grandparent, … up to the scene root.
pub fn ancestors_or_self(world: &World, id: u32) -> Vec<u32> {
    let mut chain = Vec::new();
    let mut cur = Some(id);
    while let Some(c) = cur {
        if chain.len() > world.len() || !world.contains(c) {
            break;
        }
        chain.push(c);
        cur = world.parent_id(c);
    }
    chain
}

/// Whether `id` and every ancestor are active — the rule drawing uses too.
pub fn is_visible(world: &World, id: u32) -> bool {
    world.contains(id)
        && ancestors_or_self(world, id)
            .into_iter()
            .all(|c| world.is_active(c))
}

/// Whether a `CanvasGroup` with `blocks_raycasts = false` sits on `id` or above it
/// (the pointer passes through that whole subtree).
pub fn raycasts_blocked(world: &World, id: u32) -> bool {
    ancestors_or_self(world, id)
        .into_iter()
        .any(|c| world.canvas_group(c).is_some_and(|g| !g.blocks_raycasts))
}

/// Whether `id` accepts input: not a Selectable with `interactable = false`, and no
/// `CanvasGroup` with `interactable = false` on it or above it. An entity without a
/// Selectable is interactable unless a group disables it.
pub fn is_interactable(world: &World, id: u32) -> bool {
    world.selectable(id).is_none_or(|s| s.interactable)
        && !ancestors_or_self(world, id)
            .into_iter()
            .any(|c| world.canvas_group(c).is_some_and(|g| !g.interactable))
}

/// The nearest ancestor-or-self of `id` passing `pred` — Unity's
/// `ExecuteEvents.GetEventHandler`, the walk every bubbled event takes.
pub fn nearest(world: &World, id: u32, pred: impl Fn(u32) -> bool) -> Option<u32> {
    ancestors_or_self(world, id).into_iter().find(|&c| pred(c))
}

/// Whether `id` is `ancestor` or below it.
pub fn is_under(world: &World, id: u32, ancestor: u32) -> bool {
    ancestors_or_self(world, id).contains(&ancestor)
}
