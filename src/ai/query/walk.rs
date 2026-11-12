//! Reachability and path length (#1892, decision 3): one bounded search from the
//! querier toward every item at once (`navigation::flood`, an A\* to many goals,
//! #2088) answers them all, instead of an A\* per item.
//!
//! Split in two so the search can be time-sliced (#2116): [`FloodAsk::new`] reads
//! what the search needs off the world when the test runs, and the search itself
//! needs only the navmesh, synchronously ([`FloodAsk::lengths`]) or stepped across
//! ticks by the query queue (`queue`), which hands its finished
//! [`FloodSearch`] to [`FloodAsk::read`].

use glam::Vec3;

use super::spec::Anchor;
use super::QueryWorld;
use crate::navigation::{FloodSearch, NavigationGraph, ALL_AREAS};

/// How far past the farthest item the flood may wander, as a factor of that
/// straight-line distance: a path around a wall or up a stair is longer than the
/// crow flies, but rarely twice as long.
const SLACK: f32 = 2.0;
/// And at least this much, metres, so an item next to the querier behind a short
/// wall is still found.
const MIN_REACH: f32 = 4.0;

/// The search a `path` test needs: from where, toward which items, how far, and
/// over which areas. Never empty: a test with no items left asks no search.
#[derive(Clone, Debug, PartialEq)]
pub struct FloodAsk {
    pub from: Vec3,
    pub items: Vec<Vec3>,
    /// The bound, metres of travel cost.
    pub bound: f32,
    pub mask: u32,
}

impl FloodAsk {
    /// The search for `items` from `querier`: the querier's own areas when it is
    /// an agent, every area otherwise. `None` with no items.
    pub fn new(world: &QueryWorld, querier: Anchor, items: &[Vec3]) -> Option<Self> {
        let mask = querier
            .entity
            .and_then(|id| world.scene.world.nav_agent(id).map(|a| a.area_mask))
            .unwrap_or(ALL_AREAS);
        Self::with_mask(querier.position, items, mask)
    }

    /// The search for `items` from `from` over the areas in `mask`; `None` with
    /// no items.
    pub fn with_mask(from: Vec3, items: &[Vec3], mask: u32) -> Option<Self> {
        if items.is_empty() {
            return None;
        }
        let farthest = items.iter().map(|p| p.distance(from)).fold(0.0, f32::max);
        Some(Self {
            from,
            items: items.to_vec(),
            bound: (farthest * SLACK).max(farthest + MIN_REACH),
            mask,
        })
    }

    /// Begin `search` on this ask ([`NavigationGraph::begin_flood`]).
    pub fn begin(&self, nav: &NavigationGraph, search: &mut FloodSearch) {
        nav.begin_flood(search, self.from, &self.items, self.bound, self.mask);
    }

    /// Each item's travel cost, run to the end now: the search a synchronous
    /// query makes.
    pub fn lengths(&self, nav: &NavigationGraph) -> Vec<Option<f32>> {
        match nav.flood_to(self.from, &self.items, self.bound, self.mask) {
            Some(flood) => self
                .items
                .iter()
                .map(|&p| flood.length_to(nav, p))
                .collect(),
            None => vec![None; self.items.len()],
        }
    }

    /// Each item's travel cost from a finished `search` begun on this ask, metres,
    /// or `None` for an item it cannot reach (or one past the bound).
    pub fn read(&self, nav: &NavigationGraph, search: &FloodSearch) -> Vec<Option<f32>> {
        self.items
            .iter()
            .map(|&p| search.length_to(nav, p))
            .collect()
    }
}
