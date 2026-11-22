//! Detour's `dtNodePool`. Each slot carries a **generation stamp**: a query bumps the generation
//! and an older slot reads as unreached, so starting a query is O(1) instead of filling
//! navmesh-sized arrays. A nested search finds the scratch borrowed and runs on a fresh pool. A
//! stale slot reads exactly as a fresh one, so reuse keeps paths byte-identical.

use super::SpanRef;
use std::cell::RefCell;

#[derive(Clone, Copy)]
struct Slot {
    stamp: u32,
    g: f32,
    from: Option<SpanRef>,
}

const UNREACHED: Slot = Slot {
    stamp: 0,
    g: f32::INFINITY,
    from: None,
};

#[derive(Default)]
pub(crate) struct NodePool {
    slots: Vec<Slot>,
    /// Never 0, which only [`UNREACHED`] carries.
    generation: u32,
}

impl NodePool {
    pub(crate) fn begin(&mut self, spans: usize) {
        if self.slots.len() < spans {
            self.slots.resize(spans, UNREACHED);
        }
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            // Wrapped after 2³² queries: clear the stamps once so no old slot
            // can pass for the new generation.
            self.slots.fill(UNREACHED);
            self.generation = 1;
        }
    }

    fn slot(&self, index: u32) -> Slot {
        match self.slots[index as usize] {
            s if s.stamp == self.generation => s,
            _ => UNREACHED,
        }
    }

    pub(crate) fn covers(&self, index: u32) -> bool {
        (index as usize) < self.slots.len()
    }

    pub(crate) fn g(&self, index: u32) -> f32 {
        self.slot(index).g
    }

    pub(crate) fn came_from(&self, index: u32) -> Option<SpanRef> {
        self.slot(index).from
    }

    pub(crate) fn set(&mut self, index: u32, g: f32, from: Option<SpanRef>) {
        self.slots[index as usize] = Slot {
            stamp: self.generation,
            g,
            from,
        };
    }
}

impl NodePool {
    pub(crate) fn remap(&mut self, spans: usize, remap: &dyn Fn(u32) -> Option<SpanRef>) -> bool {
        let generation = self.generation;
        let mut moved = Vec::new();
        for (i, s) in self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.stamp == generation)
        {
            let at = remap(i as u32).map(|r| r.index);
            let from = s.from.map(|f| remap(f.index));
            let (Some(at), None | Some(Some(_))) = (at, from) else {
                return false;
            };
            moved.push((at, s.g, from.flatten()));
        }
        self.begin(spans);
        for (at, g, from) in moved {
            self.set(at, g, from);
        }
        true
    }
}

#[derive(Default)]
pub(crate) struct SearchScratch(RefCell<NodePool>);

impl SearchScratch {
    pub(crate) fn with<R>(&self, spans: usize, search: impl FnOnce(&mut NodePool) -> R) -> R {
        let run = |pool: &mut NodePool| {
            pool.begin(spans);
            search(pool)
        };
        match self.0.try_borrow_mut() {
            Ok(mut pool) => run(&mut pool),
            Err(_) => run(&mut NodePool::default()),
        }
    }

    /// A search that runs meanwhile works on the empty pool left in its place.
    pub(crate) fn take(&self, spans: usize) -> NodePool {
        let mut pool = match self.0.try_borrow_mut() {
            Ok(mut shared) => std::mem::take(&mut *shared),
            Err(_) => NodePool::default(),
        };
        pool.begin(spans);
        pool
    }

    pub(crate) fn restore(&self, pool: NodePool) {
        if let Ok(mut shared) = self.0.try_borrow_mut() {
            if shared.slots.len() < pool.slots.len() {
                *shared = pool;
            }
        }
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
