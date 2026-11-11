//! Goals no search can reach (#2087). Most of a bake's spans join one floor, but
//! wall tops, roofs and crate tops are islands of their own, and a target sampled
//! onto one sends A\* flooding everything it can reach before it settles for a
//! partial path: on the Dust2 arena that is the whole [`MAX_SEARCH_NODES`], sixty
//! ticks of the path queue's budget, with every agent behind it waiting.
//!
//! So each bake's islands are worked out once, lazily: the walking areas, joined
//! by A\*'s own eight grid steps (`NavigationGraph::neighbour`: a diagonal into a
//! sunken cell no cardinal step reaches still joins it), taken both ways, and the
//! off-mesh links between them, followed one way. A diagonal step can be one-way;
//! joining it both ways only merges areas, which errs toward the full cap. A goal whose area no chain of
//! links leads to from the start's is unreachable for any area mask (masks only
//! remove moves), and its search is capped at [`UNREACHABLE_SEARCH_NODES`]: the
//! partial path still heads toward it, as far as that many expansions get (Detour's
//! `maxNodes` answer).

use std::cell::RefCell;
use std::collections::VecDeque;

use super::super::astar::MAX_SEARCH_NODES;
use super::super::{NavigationGraph, SpanRef};

/// The expansions a search toward a provably unreachable goal may spend: a few
/// ticks of the default budget, room enough to walk the partial path well toward
/// it in A\*'s order.
pub const UNREACHABLE_SEARCH_NODES: u32 = 4_096;

/// One bake's islands: each span's walking area, and the areas each one's
/// off-mesh links lead to.
struct IslandMap {
    generation: u64,
    area: Vec<u32>,
    exits: Vec<Vec<u32>>,
}

/// The graph's cached [`IslandMap`], rebuilt when the bake changes.
#[derive(Default)]
pub(crate) struct Islands(RefCell<Option<IslandMap>>);

impl NavigationGraph {
    /// The expansions a search from `start` to `goal` may spend:
    /// [`MAX_SEARCH_NODES`], or [`UNREACHABLE_SEARCH_NODES`] when no walk and no
    /// link joins them.
    pub(crate) fn search_cap(&self, start: SpanRef, goal: SpanRef) -> u32 {
        match self.may_reach(start, goal) {
            true => MAX_SEARCH_NODES,
            false => UNREACHABLE_SEARCH_NODES,
        }
    }

    /// Whether some walk and chain of off-mesh links leads from `start` to `goal`,
    /// whatever the area mask. `true` whenever it cannot tell (the cache in use).
    fn may_reach(&self, start: SpanRef, goal: SpanRef) -> bool {
        let Ok(mut cache) = self.islands.0.try_borrow_mut() else {
            return true;
        };
        let stale = |m: &IslandMap| {
            m.generation != self.bake_generation || m.area.len() != self.spans.len()
        };
        if cache.as_ref().is_none_or(stale) {
            *cache = Some(self.island_map());
        }
        let Some(map) = cache.as_ref() else {
            return true;
        };
        let (from, to) = (
            map.area[start.index as usize],
            map.area[goal.index as usize],
        );
        let mut seen = vec![false; map.exits.len()];
        let mut queue = VecDeque::from([from]);
        seen[from as usize] = true;
        while let Some(a) = queue.pop_front() {
            if a == to {
                return true;
            }
            for &b in &map.exits[a as usize] {
                if !std::mem::replace(&mut seen[b as usize], true) {
                    queue.push_back(b);
                }
            }
        }
        false
    }

    /// The current bake's [`IslandMap`].
    fn island_map(&self) -> IslandMap {
        let (area, count) = self.step_areas();
        let mut exits = vec![Vec::new(); count];
        for here in self.span_refs() {
            let a = area[here.index as usize];
            for link in self.link_moves(here) {
                let b = area[link.to.index as usize];
                if b != a && !exits[a as usize].contains(&b) {
                    exits[a as usize].push(b);
                }
            }
        }
        IslandMap {
            generation: self.bake_generation,
            area,
            exits,
        }
    }

    /// Each span's walking area under A\*'s steps (see the module doc), numbered in
    /// scan order, and how many there are. The cardinal steps join first; a
    /// diagonal is tried only into a cell holding a span of another area, so open
    /// floor pays no more than the cardinals.
    fn step_areas(&self) -> (Vec<u32>, usize) {
        let mut roots = Roots((0..self.spans.len() as u32).collect());
        for here in self.span_refs() {
            for (dx, dz) in CARDINAL {
                if let Some(next) = self.link_to(here, here.gx + dx, here.gz + dz) {
                    roots.join(here.index, next.index);
                }
            }
        }
        for here in self.span_refs() {
            for (dx, dz) in DIAGONAL {
                let own = roots.find(here.index);
                let cell = self.span_range(here.gx + dx, here.gz + dz);
                if cell.clone().all(|i| roots.find(i as u32) == own) {
                    continue;
                }
                if let Some(next) = self.neighbour(here, dx, dz) {
                    roots.join(here.index, next.index);
                }
            }
        }
        let mut area = vec![u32::MAX; self.spans.len()];
        let mut count = 0;
        for i in 0..self.spans.len() as u32 {
            let r = roots.find(i) as usize;
            if area[r] == u32::MAX {
                (area[r], count) = (count as u32, count + 1);
            }
            area[i as usize] = area[r];
        }
        (area, count)
    }
}

/// A union-find over span indices, each set rooted at its lowest index.
struct Roots(Vec<u32>);

impl Roots {
    fn find(&mut self, mut i: u32) -> u32 {
        while self.0[i as usize] != i {
            self.0[i as usize] = self.0[self.0[i as usize] as usize];
            i = self.0[i as usize];
        }
        i
    }

    fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        self.0[a.max(b) as usize] = a.min(b);
    }
}

/// The grid steps A\* takes. A step can go one way only (`link_to` picks the
/// nearest floor, a diagonal needs its own two cardinals), so every span tries
/// all eight of its own.
const CARDINAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const DIAGONAL: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

#[cfg(test)]
#[path = "islands_tests.rs"]
mod tests;
