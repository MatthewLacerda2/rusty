//! Goals no search can reach (#2087). Most of a bake's spans join one floor, but
//! wall tops, roofs and crate tops are islands of their own, and a target sampled
//! onto one sends A\* flooding everything it can reach before it settles for a
//! partial path: on the Dust2 arena that is the whole [`MAX_SEARCH_NODES`], sixty
//! ticks of the path queue's budget, with every agent behind it waiting.
//!
//! So the navmesh's islands are worked out by the first search that needs them,
//! and kept: the walking areas, joined
//! by A\*'s own eight grid steps (`NavigationGraph::neighbour`: a diagonal into a
//! sunken cell no cardinal step reaches still joins it), taken both ways, and the
//! off-mesh links between them, followed one way. A diagonal step can be one-way;
//! joining it both ways only merges areas, which errs toward the full cap. A goal whose area no chain of
//! links leads to from the start's is unreachable for any area mask (masks only
//! remove moves), and its search is capped at [`UNREACHABLE_SEARCH_NODES`]: the
//! partial path still heads toward it, as far as that many expansions get (Detour's
//! `maxNodes` answer). The environment query's flood (`flood`, #2088) asks the
//! same map which of its goals to drop before it starts.
//!
//! The map is built from scratch after a full bake; an incremental rebake and an
//! area-cost change carry it forward instead (`carry`), which may join areas a
//! fresh build would keep apart, never the reverse, until the navmesh has stood
//! still for a second and a carried map is built fresh again. So what the map
//! holds depends on the rebake history, not only the current bake: still a pure
//! function of the sim's inputs. Split: `areas` joins the walking areas, `carry`
//! carries them.

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

/// One bake's islands: each span's walking area, and the areas each one's
/// off-mesh links lead to.
struct IslandMap {
    /// The bake it answers for: the current one, or an older one carried forward.
    generation: u64,
    area: Vec<u32>,
    exits: Vec<Vec<u32>>,
    /// Carried across a rebake since it was last built: it may join areas a fresh
    /// build keeps apart, so a quiet spell rebuilds it (`carry::QUIET_SYNCS`).
    carried: bool,
}

/// The graph's cached [`IslandMap`]: carried across an incremental rebake and an
/// area-cost change (`carry`), rebuilt after anything else, and after a carried map
/// has stood still a while. Not a pure function of the bake: what it holds depends
/// on the rebake history and on the syncs since, all of them sim inputs, so it is
/// deterministic.
#[derive(Default)]
pub(crate) struct Islands {
    map: RefCell<Option<IslandMap>>,
    /// Syncs without a change since the map was last carried (`Islands::still`).
    quiet: u32,
    /// How many times the map was built from scratch (a test probe).
    #[cfg(test)]
    pub(crate) builds: std::cell::Cell<u32>,
}

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
        self.reach(start, std::slice::from_ref(&goal))
            .is_none_or(|reach| reach.has(goal))
    }

    /// Keep only the `goals` some walk and chain of off-mesh links may lead to
    /// from `start`, whatever the area mask: the flood (#2088) drops the rest
    /// before it starts. Keeps them all whenever it cannot tell (the cache in use).
    pub(crate) fn retain_reachable(&self, start: SpanRef, goals: &mut Vec<SpanRef>) {
        if let Some(reach) = self.reach(start, goals) {
            goals.retain(|&g| reach.has(g));
        }
    }

    /// The areas a walk and chain of links from `start` leads to, walked only as
    /// far as every one of `goals`' areas is found. `None` when the cache is in
    /// use.
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

    /// The current bake's [`IslandMap`], built from scratch.
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

    /// The areas each of the `count` areas' off-mesh links lead to, spans in
    /// `area`'s: one pass over the links, both ways for a two-way one.
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

/// The areas [`NavigationGraph::reach`] found, holding the cached map they index.
struct Reach<'a> {
    map: RefMut<'a, IslandMap>,
    seen: Vec<bool>,
}

impl Reach<'_> {
    /// Whether the walk found `span`'s area.
    fn has(&self, span: SpanRef) -> bool {
        self.seen[self.map.area[span.index as usize] as usize]
    }
}

#[cfg(test)]
mod tests;
