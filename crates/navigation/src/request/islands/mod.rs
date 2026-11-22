//! A goal whose area no chain of links leads to from the start's is unreachable for any area mask
//! (masks only remove moves), so its search is capped at [`UNREACHABLE_SEARCH_NODES`]. Areas join
//! by A*'s own eight steps taken both ways, so a one-way diagonal only merges areas, erring toward
//! the full cap.

use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;

mod areas;
mod carry;

use super::super::astar::MAX_SEARCH_NODES;
use super::super::{NavigationGraph, SpanRef};

/// The expansions a search toward a provably unreachable goal may spend: a few
/// ticks of the default budget, room enough to walk the partial path well toward
/// it in A\*'s order.
pub const UNREACHABLE_SEARCH_NODES: u32 = 4_096;

struct IslandMap {
    generation: u64,
    area: Vec<u32>,
    exits: Vec<Vec<u32>>,
    carried: bool,
}

#[derive(Default)]
pub(crate) struct Islands {
    map: RefCell<Option<IslandMap>>,
    quiet: u32,
    #[cfg(test)]
    pub(crate) builds: std::cell::Cell<u32>,
}

impl NavigationGraph {
    pub(crate) fn search_cap(&self, start: SpanRef, goal: SpanRef) -> u32 {
        match self.may_reach(start, goal) {
            true => MAX_SEARCH_NODES,
            false => UNREACHABLE_SEARCH_NODES,
        }
    }

    fn may_reach(&self, start: SpanRef, goal: SpanRef) -> bool {
        self.reach(start, std::slice::from_ref(&goal))
            .is_none_or(|reach| reach.has(goal))
    }

    pub(crate) fn retain_reachable(&self, start: SpanRef, goals: &mut Vec<SpanRef>) {
        if let Some(reach) = self.reach(start, goals) {
            goals.retain(|&g| reach.has(g));
        }
    }

    fn reach<'a>(&'a self, start: SpanRef, goals: &[SpanRef]) -> Option<Reach<'a>> {
        let mut cache = self.islands.map.try_borrow_mut().ok()?;
        let stale = |m: &IslandMap| {
            m.generation != self.bake_generation || m.area.len() != self.spans.len()
        };
        if cache.as_ref().is_none_or(stale) {
            *cache = Some(self.island_map());
        }
        let map = RefMut::filter_map(cache, Option::as_mut).ok()?;
        let area = |s: SpanRef| map.area[s.index as usize];
        let mut seen = vec![false; map.exits.len()];
        let mut queue = VecDeque::from([area(start)]);
        seen[area(start) as usize] = true;
        let mut left = goals.iter().filter(|&&g| !seen[area(g) as usize]).count();
        while let Some(a) = queue.pop_front().filter(|_| left > 0) {
            for &b in &map.exits[a as usize] {
                if !std::mem::replace(&mut seen[b as usize], true) {
                    left -= goals.iter().filter(|&&g| area(g) == b).count();
                    queue.push_back(b);
                }
            }
        }
        Some(Reach { map, seen })
    }

    fn island_map(&self) -> IslandMap {
        #[cfg(test)]
        self.islands.builds.set(self.islands.builds.get() + 1);
        let (area, count) = self.step_areas();
        let exits = self.link_exits(&area, count);
        IslandMap {
            generation: self.bake_generation,
            area,
            exits,
            carried: false,
        }
    }

    fn link_exits(&self, area: &[u32], count: usize) -> Vec<Vec<u32>> {
        let mut exits = vec![Vec::new(); count];
        let mut exit = |a: u32, b: u32| {
            if a != b && !exits[a as usize].contains(&b) {
                exits[a as usize].push(b);
            }
        };
        let links = &self.offmesh;
        for l in links.auto.iter().chain(&links.authored) {
            let ends = (self.link_span(l.from), self.link_span(l.to));
            let (Some(from), Some(to)) = ends else {
                continue;
            };
            let (a, b) = (area[from.index as usize], area[to.index as usize]);
            exit(a, b);
            if l.bidirectional {
                exit(b, a);
            }
        }
        exits
    }
}

struct Reach<'a> {
    map: RefMut<'a, IslandMap>,
    seen: Vec<bool>,
}

impl Reach<'_> {
    fn has(&self, span: SpanRef) -> bool {
        self.seen[self.map.area[span.index as usize] as usize]
    }
}

#[cfg(test)]
mod tests;
