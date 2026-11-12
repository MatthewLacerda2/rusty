//! src/navigation/flood/mod.rs — one bounded search from a point to many goals (#1892, #2088).
//!
//! An environment query's `path` test asks, for every candidate spot, whether the
//! querier can walk there and how far. One A\* per item is what Unreal pays; a
//! single search out from the querier answers every item at once. It walks the
//! same moves and costs as [`NavigationGraph::find_path_masked`] (8-way span
//! links, the diagonal corner rule, area costs, off-mesh links, the area mask with
//! the start span exempt), and stops at `max_length`, so it costs the area it
//! covers, not the level.
//!
//! With goals it is an A\* toward all of them at once (`goals`): the key is the
//! cost so far plus the octile distance to the nearest goal not yet settled, so it
//! searches a corridor toward the items instead of a disc as wide as the farthest
//! one. That heuristic never overestimates and is shrunk for `f32` rounding, so
//! each goal's cost is the very `f32` a plain Dijkstra flood (no goals) gives it.
//! A span whose key is past the bound leads to no goal within it, and is not
//! entered. Goals no walk or link chain leads to are dropped up front, from the
//! bake's island map (`request::islands`).
//!
//! Its costs live in the graph's stamped search pool (`pool`, #2083), as A\*'s do:
//! starting a flood costs nothing per span, and a cost is an array read, not a
//! hash. The [`Flood`] holds the pool while it is read and hands it back when
//! dropped. Which spans an off-mesh link leaves from is kept on the graph until
//! the next bake (`cache`). The search itself is resumable (`search`, #2116): the
//! query's time-sliced form steps one across ticks on a budget of expansions, and
//! carries it across an incremental rebake that leaves it standing (`carry`).

use glam::Vec3;

use super::NavigationGraph;

pub(super) use cache::FloodCache;
pub use search::FloodSearch;

mod cache;
mod carry;
mod goals;
mod moves;
mod search;

/// The spans a flood reached, each with its travel cost from the start. Holds
/// the graph's search pool until it is dropped: a search that runs meanwhile
/// works on a fresh one.
pub struct Flood<'a> {
    nav: &'a NavigationGraph,
    /// The finished search, its costs in the graph's pool.
    search: FloodSearch,
}

impl Flood<'_> {
    /// The travel cost from the flood's start to the span `p` snaps to, metres
    /// (area costs weighed in, as the planner weighs them), or `None` when the
    /// flood never reached it: unreachable, or past its bound.
    pub fn length_to(&self, nav: &NavigationGraph, p: Vec3) -> Option<f32> {
        self.search.length_to(nav, p)
    }

    /// How many spans the flood reached.
    pub fn reached(&self) -> usize {
        self.search.reached
    }
}

impl Drop for Flood<'_> {
    fn drop(&mut self) {
        let pool = std::mem::take(&mut self.search.cost);
        self.nav.search.restore(pool);
    }
}

impl NavigationGraph {
    /// Flood out from the span `start` snaps to, entering only the areas in
    /// `mask`, as far as `max_length` metres of travel cost: a Dijkstra flood
    /// that leaves every span within the bound at its exact cost. `None` when
    /// `start` is off the navmesh.
    pub fn flood(&self, start: Vec3, max_length: f32, mask: u32) -> Option<Flood<'_>> {
        self.flood_to(start, &[], max_length, mask)
    }

    /// [`Self::flood`] headed for the spans `goals` snap to, stopping once each
    /// is settled (its cost final) or proven out of reach: the environment query
    /// asks about a few dozen spots, and the flood around them is most of its
    /// cost. Exact for the goals, to the bit; another span it reports may hold a
    /// cost a full flood would still lower, or be left unreached. A goal off the
    /// navmesh is ignored; with none at all, it is [`Self::flood`]. It is a
    /// [`FloodSearch`] run to its end in the call (`search`).
    pub fn flood_to(
        &self,
        start: Vec3,
        goals: &[Vec3],
        max_length: f32,
        mask: u32,
    ) -> Option<Flood<'_>> {
        self.snap(start)?;
        let mut search = FloodSearch::with_pool(self.search.take(self.spans.len()));
        self.begin_flood(&mut search, start, goals, max_length, mask);
        let mut unlimited = u32::MAX;
        self.step_flood(&mut search, &mut unlimited);
        Some(Flood { nav: self, search })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod island_tests;

#[cfg(test)]
mod order_tests;

// The bench level it runs on is a `dev` test fixture.
#[cfg(all(test, feature = "dev"))]
mod exact_tests;

#[cfg(test)]
mod slice_tests;
