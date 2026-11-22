//! `activeInHierarchy`, answered once per id and kept; a read cleans the chain above it. A flag or
//! parent-link change dirties its subtree down `children`; an inserted, overwritten or restored
//! document bumps the generation, since an orphan can name it from no `children` list. Stamps
//! gate the cache, never an answer.

use std::cell::RefCell;

use super::bundle::Core;
use super::world::World;

pub(crate) struct ActivityStore {
    inner: RefCell<Flags>,
    /// A parent link was written without its `children` entry: subtree walks can miss a
    /// descendant, so every invalidation bumps the whole store.
    unlisted: bool,
}

struct Flags {
    flags: Vec<(u32, bool)>,
    /// Never `0`, so a fresh entry reads as dirty.
    generation: u32,
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
    pub(crate) fn invalidate_all(&self) {
        let mut store = self.inner.borrow_mut();
        store.generation = store.generation.wrapping_add(1);
        if store.generation == 0 {
            // Wrapped: an old stamp could alias the new generation.
            store.flags.clear();
            store.generation = 1;
        }
    }

    pub(crate) fn forget_dead(&self, id: u32) {
        match self.unlisted {
            true => self.invalidate_all(),
            false => self.invalidate(id),
        }
    }

    fn invalidate(&self, id: u32) {
        if let Some(flag) = self.inner.borrow_mut().flags.get_mut(id as usize) {
            flag.0 = 0;
        }
    }

    #[cfg(test)]
    pub(crate) fn is_clean(&self, id: u32) -> bool {
        self.inner.borrow().clean(id).is_some()
    }

    #[cfg(test)]
    pub(crate) fn is_unlisted(&self) -> bool {
        self.unlisted
    }

    pub(crate) fn reset(&mut self) {
        *self.inner.borrow_mut() = Flags::new();
        self.unlisted = false;
    }

    pub(crate) fn mark_unlisted(&mut self) {
        self.unlisted = true;
    }

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
    pub(crate) fn invalidate_caches(&self) {
        self.world_space.invalidate_all();
        self.activity.invalidate_all();
    }

    pub(crate) fn relinked_activity(&mut self, id: u32, parent: Option<u32>) {
        if parent.is_some_and(|p| !self.lists_child(p, id)) {
            self.activity.mark_unlisted();
        }
        self.invalidate_activity_below(id);
    }

    pub(crate) fn lists_child(&self, parent: u32, child: u32) -> bool {
        self.component::<Core>(parent)
            .is_some_and(|c| c.children.contains(&child))
    }

    pub(crate) fn invalidate_activity_below(&self, id: u32) {
        if self.activity.unlisted {
            return self.activity.invalidate_all();
        }
        for e in self.self_and_descendants(id) {
            self.activity.invalidate(e);
        }
    }

    pub(crate) fn reset_caches(&mut self) {
        self.world_space.reset();
        self.activity.reset();
    }
}
