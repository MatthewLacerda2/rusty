//! Invariant: a dirty entity's descendants are all dirty, so a clean matrix was built from clean
//! ancestors. A write only queues; the next read drains the queue, stopping at entities already
//! dirty. A change the downward walk cannot see (an insert an orphan names, an overwrite) bumps
//! the generation. A child linked one side only can read stale, never NaN. Stamps gate the
//! cache, never a matrix.

use std::cell::RefCell;

use glam::Mat4;

use crate::World;

#[derive(Clone, Copy)]
struct Slot {
    world: Mat4,
    stamp: u32,
    queued: bool,
}

const EMPTY: Slot = Slot {
    world: Mat4::IDENTITY,
    stamp: 0,
    queued: false,
};

pub(crate) struct WorldStore {
    inner: RefCell<Slots>,
}

struct Slots {
    index: Vec<u32>,
    slots: Vec<Slot>,
    free: Vec<u32>,
    /// Never `0`, so a `0` stamp reads as dirty.
    generation: u32,
    pending: Vec<u32>,
    stack: Vec<u32>,
}

impl Default for WorldStore {
    fn default() -> Self {
        Self {
            inner: RefCell::new(Slots::new()),
        }
    }
}

impl Slots {
    fn new() -> Self {
        Self {
            index: Vec::new(),
            slots: Vec::new(),
            free: Vec::new(),
            generation: 1,
            pending: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn forget_all(&mut self) {
        self.index.clear();
        self.slots.clear();
        self.free.clear();
        self.pending.clear();
    }

    fn find(&self, id: u32) -> Option<usize> {
        let at = *self.index.get(id as usize)?;
        (at != 0).then(|| at as usize - 1)
    }

    fn clean(&self, id: u32) -> Option<Mat4> {
        let s = &self.slots[self.find(id)?];
        (s.stamp == self.generation).then_some(s.world)
    }

    fn slot(&mut self, id: u32) -> &mut Slot {
        if let Some(at) = self.find(id) {
            return &mut self.slots[at];
        }
        let at = match self.free.pop() {
            Some(at) => {
                self.slots[at as usize] = EMPTY;
                at as usize
            }
            None => {
                self.slots.push(EMPTY);
                self.slots.len() - 1
            }
        };
        let entry = id as usize;
        if entry >= self.index.len() {
            self.index.resize(entry + 1, 0);
        }
        self.index[entry] = at as u32 + 1;
        &mut self.slots[at]
    }

    fn put(&mut self, id: u32, world: Mat4) {
        let stamp = self.generation;
        let slot = self.slot(id);
        (slot.world, slot.stamp) = (world, stamp);
    }

    fn dirty(&mut self, id: u32) -> bool {
        let generation = self.generation;
        let Some(at) = self.find(id) else {
            return false;
        };
        match &mut self.slots[at] {
            s if s.stamp == generation => {
                s.stamp = 0;
                true
            }
            _ => false,
        }
    }

    fn drain(&mut self, world: &World) {
        let mut pending = std::mem::take(&mut self.pending);
        let mut stack = std::mem::take(&mut self.stack);
        for &root in &pending {
            if let Some(at) = self.find(root) {
                self.slots[at].queued = false;
            }
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
    /// No slot: no read reached it, so no descendant was cleaned through it.
    pub(crate) fn moved(&self, id: u32) {
        let mut store = self.inner.borrow_mut();
        let Some(at) = store.find(id) else {
            return;
        };
        if !std::mem::replace(&mut store.slots[at].queued, true) {
            store.pending.push(id);
        }
    }

    /// Its children must already be queued ([`WorldStore::moved`]).
    pub(crate) fn forget(&self, id: u32) {
        let mut store = self.inner.borrow_mut();
        if let Some(at) = store.find(id) {
            store.index[id as usize] = 0;
            store.free.push(at as u32);
        }
    }

    pub(crate) fn invalidate_all(&self) {
        let mut store = self.inner.borrow_mut();
        store.generation = store.generation.wrapping_add(1);
        if store.generation == 0 {
            store.forget_all();
            store.generation = 1;
        }
    }

    pub(crate) fn reset(&self) {
        *self.inner.borrow_mut() = Slots::new();
    }

    pub(crate) fn resolve(&self, world: &World, id: u32) -> Mat4 {
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
