//! src/ecs/world_space/store.rs — the world-matrix store and its dirty flags (#1170).
//!
//! One slot per stable id: the entity's world matrix and the stamp of the fill it was
//! computed in. A slot is **clean** when its stamp is the store's current `generation`;
//! anything else (a fresh slot, one marked dirty, one from before a bump) is dirty.
//!
//! **The invariant.** A clean entity's parent is clean, so a clean matrix was built from
//! clean ancestors and is never stale. Two moves keep it: a write marks the written
//! entity's whole subtree dirty ([`WorldStore::mark_dirty`] over the downward walk), and
//! a read cleans the dirty chain above an entity top-down, parents first
//! ([`WorldStore::resolve`]). A structural change that can re-point parent links the
//! downward walk cannot see (a despawn, a document inserted under an id an orphan
//! already names, a whole-entity overwrite, a `clear`) bumps the generation instead:
//! every slot goes dirty in O(1), and the next reads re-derive what they need.
//!
//! The downward walk follows `children`, which `Scene::set_parent` and rehydration keep
//! in step with the parent links. Only a corrupt file can write a parent link its
//! parent's `children` lacks; such a child reads its parent as of the last structural
//! change, never NaN and never a hang.
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
}

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
}

impl Default for WorldStore {
    fn default() -> Self {
        Self {
            inner: RefCell::new(Slots {
                slots: Vec::new(),
                generation: 1,
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

    fn put(&mut self, id: u32, world: Mat4) {
        let at = id as usize;
        if at >= self.slots.len() {
            let empty = Slot {
                world: Mat4::IDENTITY,
                stamp: 0,
            };
            self.slots.resize(at + 1, empty);
        }
        self.slots[at] = Slot {
            world,
            stamp: self.generation,
        };
    }
}

impl WorldStore {
    /// Mark `id` dirty.
    pub(in crate::ecs) fn mark_dirty(&self, id: u32) {
        if let Some(s) = self.inner.borrow_mut().slots.get_mut(id as usize) {
            s.stamp = 0;
        }
    }

    /// Every slot dirty at once.
    pub(in crate::ecs) fn invalidate_all(&self) {
        let mut store = self.inner.borrow_mut();
        store.generation = store.generation.wrapping_add(1);
        if store.generation == 0 {
            // Wrapped: an old stamp could alias the new generation, so forget them all.
            store.slots.clear();
            store.generation = 1;
        }
    }

    /// Forget every slot (a `clear`, after which ids restart at 1).
    pub(in crate::ecs) fn reset(&self) {
        self.inner.borrow_mut().slots.clear();
        self.invalidate_all();
    }

    /// `id`'s world matrix, cleaning the dirty chain above it first. Identity for a dead
    /// id. The climb is [`World::self_and_ancestors`], bounded, so a malformed parent
    /// cycle ends it instead of hanging it (#991).
    pub(in crate::ecs) fn resolve(&self, world: &World, id: u32) -> Mat4 {
        let mut store = self.inner.borrow_mut();
        if let Some(m) = store.clean(id) {
            return m;
        }
        let mut chain = Vec::new();
        let mut above = Mat4::IDENTITY;
        for c in world.self_and_ancestors(id) {
            if let Some(m) = store.clean(c) {
                above = m;
                break;
            }
            chain.push(c);
        }
        for &c in chain.iter().rev() {
            if let Some(local) = world.transform(c).map(|t| t.to_matrix()) {
                above *= local;
                store.put(c, above);
            }
        }
        above
    }
}
