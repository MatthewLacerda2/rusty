//! src/navigation/pool.rs — the per-graph search scratch (#2083), Detour's `dtNodePool`.
//!
//! A search over spans keeps a best cost and a parent per span it reaches. Filling
//! two navmesh-sized arrays for every query made a 3 m path on a Dust2 map cost what
//! the map's size does, so the arrays live here instead, sized once and grown on a
//! rebake, and each slot carries a **generation stamp**: a query bumps the pool's
//! generation, and a slot stamped with an older one reads as unreached
//! (`INFINITY` / `None`). Starting a query is O(1) and costs nothing per span.
//!
//! Ownership: the [`NavigationGraph`](super::NavigationGraph) holds one
//! [`SearchScratch`], a `RefCell` around the pool, because every caller reaches the
//! graph through `&self` on the sim's one thread (the scripts hold it in an
//! `Rc<RefCell<…>>`). A search that finds the scratch already borrowed (a search
//! nested inside another) runs on a fresh pool of its own, so no query ever sees
//! another's slots. A query that panics mid-way leaves stale stamps behind, which
//! the next query's generation hides: the pool stays usable with no cleanup.
//!
//! What a slot holds never decides a search's outcome (a stale slot reads exactly
//! as a fresh array's would), so reusing the pool keeps paths byte-identical.

use super::SpanRef;
use std::cell::RefCell;

/// One span's search state: the generation that wrote it, its best cost so far,
/// and the span it was reached from.
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

/// Best cost and parent per span for one search at a time, valid only for the
/// slots the current generation stamped. Get one from [`SearchScratch::with`].
#[derive(Default)]
pub(crate) struct NodePool {
    slots: Vec<Slot>,
    /// The running query's stamp; never 0, which only [`UNREACHED`] carries.
    generation: u32,
}

impl NodePool {
    /// Start a new search over `spans` spans: every slot reads as unreached. The
    /// path queue (#2087) keeps a pool of its own and begins it once per search.
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

    /// The best cost found to span `index` this search; `INFINITY` when unreached.
    pub(crate) fn g(&self, index: u32) -> f32 {
        self.slot(index).g
    }

    /// The span `index` was best reached from this search; `None` for the start
    /// and for a span not reached.
    pub(crate) fn came_from(&self, index: u32) -> Option<SpanRef> {
        self.slot(index).from
    }

    /// Record span `index` as reached at cost `g` from `from`.
    pub(crate) fn set(&mut self, index: u32, g: f32, from: Option<SpanRef>) {
        self.slots[index as usize] = Slot {
            stamp: self.generation,
            g,
            from,
        };
    }
}

impl NodePool {
    /// Carry this search's slots onto a rebaked navmesh of `spans` spans (#2087,
    /// `request::carry`), each span index renumbered by `remap`; `false`, changing
    /// nothing, when `remap` refuses one.
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

/// The graph's reusable [`NodePool`] (see the module doc for why a `RefCell`).
#[derive(Default)]
pub(crate) struct SearchScratch(RefCell<NodePool>);

impl SearchScratch {
    /// Run `search` on a pool begun for `spans` spans: the shared one, or a fresh
    /// one when the shared one is already in use.
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
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
