//! src/ecs/world_space/store.rs — the world-matrix store and its dirty flags (#1170).
//!
//! One slot per stable id: the entity's world matrix and the stamp of the fill it was
//! computed in. A slot is **clean** when its stamp is the store's current `generation`;
//! anything else (a fresh slot, one marked dirty, one from before a bump) is dirty.
//!
//! **The invariant.** A dirty entity's descendants are all dirty, so a clean matrix was
//! built from clean ancestors and is never stale. A write is O(1): it only queues the
//! written entity ([`WorldStore::moved`]). The next read drains the queue first
//! ([`WorldStore::resolve`]), marking each queued subtree dirty and stopping at any
//! entity already dirty, whose subtree the invariant says is dirty too. So a walk only
//! ever visits clean entities, each of which a read cleaned since: the Animator writing
//! every bone of a rig, parents first, walks the rig once, not once per bone. A read
//! then cleans the dirty chain above an entity top-down, parents first. A structural
//! change that can re-point parent links the downward walk cannot see (a document
//! inserted under an id an orphan already names, a whole-entity overwrite, a `clear`)
//! bumps the generation instead: every slot goes dirty in O(1).
//!
//! The downward walk follows `children`, which `Scene::set_parent` and rehydration keep
//! in step with the parent links. Only a corrupt file can write a parent link its
//! parent's `children` lacks; such a child reads its parent as of the last structural
//! change, never NaN and never a hang (the walk stops at dirty entities, so a cycle in
//! `children` ends it too).
//!
//! **Determinism.** A matrix is a pure function of the local transforms up its chain,
//! whatever order slots are cleaned in; stamps only gate the cache, never a result.

use std::cell::RefCell;

use glam::Mat4;

use crate::ecs::World;

#[derive(Clone, Copy)]
struct Slot {
    world: Mat4,
    stamp: u32,
    /// In `pending`, so a second write before the next read queues nothing.
    queued: bool,
}

const EMPTY: Slot = Slot {
    world: Mat4::IDENTITY,
    stamp: 0,
    queued: false,
};

/// The dirty-flagged world matrices. Interior-mutable so a `&World` read can clean what
/// it needs; the world lives on one thread (the scene is behind `Rc<RefCell>`), so the
/// cell never contends.
pub(in crate::ecs) struct WorldStore {
    inner: RefCell<Slots>,
}

struct Slots {
    slots: Vec<Slot>,
    /// The stamp a clean slot carries. Never `0`, so `0` always reads as dirty.
    generation: u32,
    /// Entities written since the last read, whose subtrees are not yet marked dirty.
    pending: Vec<u32>,
    /// The downward walk's stack, kept so a drain allocates nothing once warm.
    stack: Vec<u32>,
}

impl Default for WorldStore {
    fn default() -> Self {
        Self {
            inner: RefCell::new(Slots {
                slots: Vec::new(),
                generation: 1,
                pending: Vec::new(),
                stack: Vec::new(),
            }),
        }
    }
}

impl Slots {
    fn clean(&self, id: u32) -> Option<Mat4> {
        match self.slots.get(id as usize) {
            Some(s) if s.stamp == self.generation => Some(s.world),
            _ => None,
        }
    }

    fn slot(&mut self, id: u32) -> &mut Slot {
        let at = id as usize;
        if at >= self.slots.len() {
            self.slots.resize(at + 1, EMPTY);
        }
        &mut self.slots[at]
    }

    fn put(&mut self, id: u32, world: Mat4) {
        let stamp = self.generation;
        let slot = self.slot(id);
        (slot.world, slot.stamp) = (world, stamp);
    }

    /// Mark `id` dirty; `false` when it already was (its subtree is dirty too).
    fn dirty(&mut self, id: u32) -> bool {
        let generation = self.generation;
        match self.slots.get_mut(id as usize) {
            Some(s) if s.stamp == generation => {
                s.stamp = 0;
                true
            }
            _ => false,
        }
    }

    /// Mark every queued subtree dirty, stopping at entities already dirty.
    fn drain(&mut self, world: &World) {
        let mut pending = std::mem::take(&mut self.pending);
        let mut stack = std::mem::take(&mut self.stack);
        for &root in &pending {
            self.slot(root).queued = false;
            stack.push(root);
            while let Some(id) = stack.pop() {
                if self.dirty(id) {
                    world.push_children(id, &mut stack);
                }
            }
        }
        pending.clear();
        (self.pending, self.stack) = (pending, stack);
    }
}

impl WorldStore {
    /// `id` moved in world space: queue its subtree to go dirty before the next read.
    /// O(1), and nothing at all for an entity already queued.
    pub(in crate::ecs) fn moved(&self, id: u32) {
        let mut store = self.inner.borrow_mut();
        let slot = store.slot(id);
        if !std::mem::replace(&mut slot.queued, true) {
            store.pending.push(id);
        }
    }

    /// Every slot dirty at once.
    pub(in crate::ecs) fn invalidate_all(&self) {
        let mut store = self.inner.borrow_mut();
        store.generation = store.generation.wrapping_add(1);
        if store.generation == 0 {
            // Wrapped: an old stamp could alias the new generation, so forget them all.
            store.slots.clear();
            store.pending.clear();
            store.generation = 1;
        }
    }

    /// Forget every slot (a `clear`, after which ids restart at 1).
    pub(in crate::ecs) fn reset(&self) {
        let mut store = self.inner.borrow_mut();
        store.slots.clear();
        store.pending.clear();
        drop(store);
        self.invalidate_all();
    }

    /// `id`'s world matrix, cleaning the dirty chain above it first. Identity for a dead
    /// id. The climb is [`World::self_and_ancestors`], bounded, so a malformed parent
    /// cycle ends it instead of hanging it (#991).
    pub(in crate::ecs) fn resolve(&self, world: &World, id: u32) -> Mat4 {
        let mut store = self.inner.borrow_mut();
        if !store.pending.is_empty() {
            store.drain(world);
        }
        if let Some(m) = store.clean(id) {
            return m;
        }
        let mut chain = std::mem::take(&mut store.stack);
        let mut above = Mat4::IDENTITY;
        for c in world.self_and_ancestors(id) {
            if let Some(m) = store.clean(c) {
                above = m;
                break;
            }
            chain.push(c);
        }
        while let Some(c) = chain.pop() {
            if let Some(local) = world.transform(c).map(|t| t.to_matrix()) {
                above *= local;
                store.put(c, above);
            }
        }
        store.stack = chain;
        above
    }
}
