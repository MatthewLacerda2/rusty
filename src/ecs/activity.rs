//! src/ecs/activity.rs — `activeInHierarchy`, resolved once and kept (#1388).
//!
//! [`World::is_active_in_hierarchy`] is asked per entity by every per-frame loop (each
//! mesh pass, each script phase, the UI, audio), so the ancestor walk behind it is
//! answered once and remembered: one stamped flag per id. A flag is **clean** when its
//! stamp is the store's current `generation`; a read cleans the chain above an entity
//! top-down, so every ancestor it passes is answered for the next read too.
//!
//! **What invalidates.** Anything that can change an answer bumps the generation, O(1),
//! and every flag goes dirty at once: an `active` flag that changed, a parent link
//! written, a despawn (a dead parent ends its children's chains), a document inserted
//! or overwritten, a `clear`. These are rare next to the reads, and after one the next
//! pass re-resolves each entity once, its ancestors already answered. A per-subtree
//! dirty walk (as `world_space` does) would follow `children`, which a raw
//! `set_parent_id` does not keep in step; the bump reads only parent links, so it is
//! never stale.
//!
//! **Determinism.** An answer is a pure function of the `active` flags and parent links;
//! stamps only gate the cache, never a result.

use std::cell::RefCell;

use super::world::World;

/// The remembered answers. Interior-mutable so a `&World` read can fill them; the world
/// lives on one thread, so the cell never contends.
pub(in crate::ecs) struct ActivityStore {
    inner: RefCell<Flags>,
}

struct Flags {
    /// Stable id -> (stamp, answer); a stamp other than `generation` is dirty.
    flags: Vec<(u32, bool)>,
    /// The stamp a clean flag carries. Never `0`, so a fresh entry reads as dirty.
    generation: u32,
    /// The climb's stack, kept so a read allocates nothing once warm.
    chain: Vec<u32>,
}

impl Default for ActivityStore {
    fn default() -> Self {
        Self {
            inner: RefCell::new(Flags {
                flags: Vec::new(),
                generation: 1,
                chain: Vec::new(),
            }),
        }
    }
}

impl Flags {
    fn clean(&self, id: u32) -> Option<bool> {
        let &(stamp, active) = self.flags.get(id as usize)?;
        (stamp == self.generation).then_some(active)
    }

    fn put(&mut self, id: u32, active: bool) {
        let at = id as usize;
        if at >= self.flags.len() {
            self.flags.resize(at + 1, (0, false));
        }
        self.flags[at] = (self.generation, active);
    }
}

impl ActivityStore {
    /// Every answer dirty at once.
    pub(in crate::ecs) fn invalidate_all(&self) {
        let mut store = self.inner.borrow_mut();
        store.generation = store.generation.wrapping_add(1);
        if store.generation == 0 {
            // Wrapped: an old stamp could alias the new generation, so forget them all.
            store.flags.clear();
            store.generation = 1;
        }
    }

    /// Whether live entity `id` and every ancestor are active, cleaning the dirty chain
    /// above it first. The climb is [`World::self_and_ancestors`], bounded, so a
    /// malformed parent cycle ends it instead of hanging it (#991).
    pub(in crate::ecs) fn resolve(&self, world: &World, id: u32) -> bool {
        let mut store = self.inner.borrow_mut();
        if let Some(active) = store.clean(id) {
            return active;
        }
        let mut chain = std::mem::take(&mut store.chain);
        let mut above = None;
        for c in world.self_and_ancestors(id) {
            above = store.clean(c);
            if above.is_some() {
                break;
            }
            chain.push(c);
        }
        let reached_root = chain
            .last()
            .is_some_and(|&top| world.ancestors(top).next().is_none());
        let mut above = match above {
            Some(active) => active,
            None if reached_root => true,
            // The bound cut a parent cycle: answer it whole and remember nothing, since
            // a node near the top of the chain would see only part of the loop.
            None => chain.drain(..).all(|c| world.is_active_self(c)),
        };
        while let Some(c) = chain.pop() {
            above = above && world.is_active_self(c);
            store.put(c, above);
        }
        store.chain = chain;
        above
    }
}

impl World {
    /// Something re-pointed parent links or replaced rows wholesale: every cached world
    /// matrix and activity answer goes dirty.
    pub(in crate::ecs) fn invalidate_caches(&self) {
        self.world_space.invalidate_all();
        self.activity.invalidate_all();
    }
}
