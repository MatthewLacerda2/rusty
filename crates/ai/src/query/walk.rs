use glam::Vec3;

use super::spec::Anchor;
use super::QueryWorld;
use rusty_navigation::{FloodSearch, NavigationGraph, ALL_AREAS};

/// The flood's reach, as a factor of the farthest item's straight-line distance:
/// a path around a wall is longer, but rarely twice.
const SLACK: f32 = 2.0;
/// At least this much, metres, so an item next to the querier behind a short
/// wall is still found.
const MIN_REACH: f32 = 4.0;

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

    pub fn read(&self, nav: &NavigationGraph, search: &FloodSearch) -> Vec<Option<f32>> {
        self.items
            .iter()
            .map(|&p| search.length_to(nav, p))
            .collect()
    }
}
