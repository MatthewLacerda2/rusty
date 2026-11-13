//! crates/ecs/src/activity.rs — `activeInHierarchy`, resolved once and kept (#1388).
//!
//! [`World::is_active_in_hierarchy`] is asked per entity by every per-frame loop (each
//! mesh pass, each script phase, the UI, audio), so the ancestor walk behind it is
//! answered once and remembered: one stamped flag per id. A flag is **clean** when its
//! stamp is the store's current `generation`; a read cleans the chain above an entity
//! top-down, so every ancestor it passes is answered for the next read too.
//!
//! **What invalidates.** Only what can change an answer. An `active` flag that
//! changed, or a parent link, dirties that entity and its subtree (a walk down the
//! `children` lists, bounded like every subtree walk); a despawn dirties the dead
//! ids and, through the re-parent that makes them roots, each surviving child's
//! subtree, so a leaf's death touches its own answer alone (#1708). A document
//! inserted, overwritten or put back, and a `clear`, still dirty everything at once
//! (O(1), a generation bump): a document can bring a parent link in any order, an
//! orphan naming the new id from no `children` list.
//!
//! The subtree walks follow `children`, so they hold only while every parent link
//! has its `children` entry. `Scene::set_parent` writes the entry first, the
//! despawn's re-rooting clears both sides, and a placed document's link gets its
//! missing side, at once or when the parent arrives (`ecs::placement`, #1805), so a
//! hand-written file or prefab carrying one side is repaired as it loads. A raw
//! [`World::set_parent_id`] naming a parent whose list lacks the child (a test
//! linking one side, code bypassing `Scene::set_parent`) marks the store
//! `unlisted`, and from then until a `clear` every invalidation is the whole-store
//! bump again.
//!
//! **Determinism.** An answer is a pure function of the `active` flags and parent links;
//! stamps only gate the cache, never a result.

use std::cell::RefCell;

use super::bundle::Core;
use super::world::World;

/// The remembered answers. Interior-mutable so a `&World` read can fill them; the world
/// lives on one thread, so the cell never contends.
pub(crate) struct ActivityStore {
    inner: RefCell<Flags>,
    /// A parent link was written without its `children` entry: subtree walks can
    /// miss a descendant, so every invalidation bumps the whole store.
    unlisted: bool,
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
            inner: RefCell::new(Flags::new()),
            unlisted: false,
        }
    }
}

impl Flags {
    fn new() -> Self {
        Self {
            flags: Vec::new(),
            generation: 1,
            chain: Vec::new(),
        }
    }

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
    pub(crate) fn invalidate_all(&self) {
        let mut store = self.inner.borrow_mut();
        store.generation = store.generation.wrapping_add(1);
        if store.generation == 0 {
            // Wrapped: an old stamp could alias the new generation, so forget them all.
            store.flags.clear();
            store.generation = 1;
        }
    }

    /// Dead `id`'s answer dirty; everything, when a one-sided link may hang a
    /// descendant below it that no `children` list names.
    pub(crate) fn forget_dead(&self, id: u32) {
        match self.unlisted {
            true => self.invalidate_all(),
            false => self.invalidate(id),
        }
    }

    /// `id`'s answer dirty: the next read climbs from it again.
    fn invalidate(&self, id: u32) {
        if let Some(flag) = self.inner.borrow_mut().flags.get_mut(id as usize) {
            flag.0 = 0;
        }
    }

    /// Whether `id`'s kept answer is clean: what the cost tests check a despawn by.
    #[cfg(test)]
    pub(crate) fn is_clean(&self, id: u32) -> bool {
        self.inner.borrow().clean(id).is_some()
    }

    /// Whether a one-sided link turned the subtree walks off (module docs).
    #[cfg(test)]
    pub(crate) fn is_unlisted(&self) -> bool {
        self.unlisted
    }

    /// Forget every answer (a `clear`, after which ids restart at 1).
    pub(crate) fn reset(&mut self) {
        *self.inner.borrow_mut() = Flags::new();
        self.unlisted = false;
    }

    /// A parent link without its `children` entry was written (see the module docs).
    pub(crate) fn mark_unlisted(&mut self) {
        self.unlisted = true;
    }

    /// Whether live entity `id` and every ancestor are active, cleaning the dirty chain
    /// above it first. The climb is [`World::self_and_ancestors`], bounded, so a
    /// malformed parent cycle ends it instead of hanging it (#991).
    ///
    /// The climb stops at the first clean ancestor (its answer is the chain's) or the
    /// first inactive one (it and everything below it answer `false`, as the old walk's
    /// `all` stopped there). Every answer it fills is the one the whole walk would give,
    /// even in a cycle: a climb the bound cuts found every node of the loop active, so
    /// each of them answers `true`.
    pub(crate) fn resolve(&self, world: &World, id: u32) -> bool {
        let mut store = self.inner.borrow_mut();
        if let Some(active) = store.clean(id) {
            return active;
        }
        let mut chain = std::mem::take(&mut store.chain);
        let mut above = true;
        for c in world.self_and_ancestors(id) {
            if let Some(active) = store.clean(c) {
                above = active;
                break;
            }
            chain.push(c);
            if !world.is_active_self(c) {
                break;
            }
        }
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
    pub(crate) fn invalidate_caches(&self) {
        self.world_space.invalidate_all();
        self.activity.invalidate_all();
    }

    /// `id`'s parent link was re-pointed at `parent`: its subtree goes dirty, and the
    /// store is marked when `parent`'s `children` does not list it (module docs).
    pub(crate) fn relinked_activity(&mut self, id: u32, parent: Option<u32>) {
        if parent.is_some_and(|p| !self.lists_child(p, id)) {
            self.activity.mark_unlisted();
        }
        self.invalidate_activity_below(id);
    }

    /// Whether `parent`'s `children` names `child`, read in place.
    pub(crate) fn lists_child(&self, parent: u32, child: u32) -> bool {
        self.component::<Core>(parent)
            .is_some_and(|c| c.children.contains(&child))
    }

    /// `id`'s activity answer and every descendant's dirty (an `active` flag or a
    /// parent link changed there). See the module docs.
    pub(crate) fn invalidate_activity_below(&self, id: u32) {
        if self.activity.unlisted {
            return self.activity.invalidate_all();
        }
        for e in self.self_and_descendants(id) {
            self.activity.invalidate(e);
        }
    }

    /// Every cached world matrix and activity answer forgotten (a `clear`; the change
    /// logs' new epoch moves `World::static_revision` too).
    pub(crate) fn reset_caches(&mut self) {
        self.world_space.reset();
        self.activity.reset();
    }
}
