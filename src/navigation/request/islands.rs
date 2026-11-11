//! Goals no search can reach (#2087). Most of a bake's spans join one floor, but
//! wall tops, roofs and crate tops are islands of their own, and a target sampled
//! onto one sends A\* flooding everything it can reach before it settles for a
//! partial path: on the Dust2 arena that is the whole [`MAX_SEARCH_NODES`], sixty
//! ticks of the path queue's budget, with every agent behind it waiting.
//!
//! So each bake's islands are worked out once, lazily: the walking areas
//! (`NavigationGraph::walk_areas`, joined by grid steps, which go both ways) and the
//! off-mesh links between them, followed one way. A goal whose area no chain of
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
        let (area, sizes) = self.walk_areas();
        let mut exits = vec![Vec::new(); sizes.len()];
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
}

#[cfg(test)]
#[path = "islands_tests.rs"]
mod tests;
