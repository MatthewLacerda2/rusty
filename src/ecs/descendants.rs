//! src/ecs/descendants.rs — the one bounded downward hierarchy walk (#991).
//!
//! The counterpart of [`World::ancestors`]: every subtree walk (prefab extract and
//! instance membership, skeleton despawn, ragdoll bones, selection bounds, the
//! load survivors) goes through here. Each id is visited at most once, so a
//! malformed `children` list — a cycle, or a child listed twice — ends the walk
//! instead of hanging it, and a child id no longer in the world is skipped.

use std::collections::BTreeSet;

use super::world::World;

/// A pre-order walk of a subtree, children in their declared order. Built by
/// [`World::self_and_descendants`] / [`World::self_and_descendants_while`].
pub struct Descendants<'w, F> {
    world: &'w World,
    stack: Vec<u32>,
    seen: BTreeSet<u32>,
    descend: F,
}

impl<F: FnMut(u32) -> bool> Iterator for Descendants<'_, F> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        loop {
            let id = self.stack.pop()?;
            if !self.world.contains(id) || !self.seen.insert(id) {
                continue;
            }
            if (self.descend)(id) {
                // Reversed onto the stack so they come off in declared order.
                self.stack.extend(self.world.children(id).into_iter().rev());
            }
            return Some(id);
        }
    }
}

impl World {
    /// `id`, then every descendant, pre-order in declared child order; nothing for a
    /// dead id. See the module note for what a malformed hierarchy does.
    pub fn self_and_descendants(&self, id: u32) -> Descendants<'_, fn(u32) -> bool> {
        self.self_and_descendants_while(id, |_| true)
    }

    /// [`World::self_and_descendants`], but the walk goes below an entity only when
    /// `descend` says so — the entity itself is still yielded. A prefab instance uses
    /// it to stop at entities that are not its own.
    pub fn self_and_descendants_while<F: FnMut(u32) -> bool>(
        &self,
        id: u32,
        descend: F,
    ) -> Descendants<'_, F> {
        Descendants {
            world: self,
            stack: vec![id],
            seen: BTreeSet::new(),
            descend,
        }
    }
}
