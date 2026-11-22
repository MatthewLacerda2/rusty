//! One A\* per item is what Unreal pays; a single search out from the querier answers every item at
//! once. Same moves and costs as [`NavigationGraph::find_path_masked`] (start span exempt from the
//! mask), stopping at `max_length`. With goals it is A\* toward all of them: the heuristic never
//! overestimates and is shrunk for `f32` rounding, so each goal's cost is the very `f32` a plain
//! Dijkstra flood gives it.

use glam::Vec3;

use super::NavigationGraph;

pub(super) use cache::FloodCache;
pub use search::FloodSearch;

mod cache;
mod carry;
mod goals;
mod moves;
mod search;

pub struct Flood<'a> {
    nav: &'a NavigationGraph,
    search: FloodSearch,
}

impl Flood<'_> {
    /// Metres, area costs weighed in.
    pub fn length_to(&self, nav: &NavigationGraph, p: Vec3) -> Option<f32> {
        self.search.length_to(nav, p)
    }

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
    pub fn flood(&self, start: Vec3, max_length: f32, mask: u32) -> Option<Flood<'_>> {
        self.flood_to(start, &[], max_length, mask)
    }

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

#[cfg(all(test, feature = "dev"))]
mod exact_tests;

#[cfg(test)]
mod slice_tests;
