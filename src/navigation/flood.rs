//! src/navigation/flood.rs — one bounded Dijkstra flood from a point (#1892).
//!
//! An environment query's `path` test asks, for every candidate spot, whether the
//! querier can walk there and how far. One A\* per item is what Unreal pays; a
//! single flood out from the querier answers every item at once. The flood walks
//! the same moves and costs as [`NavigationGraph::find_path_masked`] (8-way span
//! links, the diagonal corner rule, area costs, off-mesh links, the area mask with
//! the start span exempt), but with no goal and no heuristic, and stops at
//! `max_length`, so it costs the area it covers, not the level.
//!
//! The reached set is a hash map keyed by span index, not a dense per-span array:
//! a flood bounded to a few dozen metres touches a sliver of a Dust2-sized grid,
//! and the dense arrays A\* allocates would cost the whole grid every query.

use std::collections::BinaryHeap;

use glam::Vec3;

use super::{in_mask, NavSpan, NavigationGraph, SpanRef, MIN_AREA_COST};
use crate::core::collections::Map;

/// The spans a flood reached, each with its travel cost from the start.
pub struct Flood {
    /// Cost from the start span, in A\*'s units (cells × area cost), by span index.
    cost: Map<u32, f32>,
    /// The grid's cell size, metres: turns a cost into metres.
    spacing: f32,
}

impl Flood {
    /// The travel cost from the flood's start to the span `p` snaps to, metres
    /// (area costs weighed in, as the planner weighs them), or `None` when the
    /// flood never reached it: unreachable, or past its bound.
    pub fn length_to(&self, nav: &NavigationGraph, p: Vec3) -> Option<f32> {
        let span = nav.snap(p)?;
        self.cost.get(&span.index).map(|c| c * self.spacing)
    }

    /// How many spans the flood reached.
    pub fn reached(&self) -> usize {
        self.cost.len()
    }
}

#[derive(Copy, Clone, PartialEq)]
struct Open {
    node: SpanRef,
    g: f32,
}

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Min-heap on cost; ties on the span index, so the pop order is fixed.
        other
            .g
            .total_cmp(&self.g)
            .then(other.node.index.cmp(&self.node.index))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The 8-way moves: (dx, dz, horizontal cost in cells), as A\* takes them.
const DIRS: [(i32, i32, f32); 8] = [
    (1, 0, 1.0),
    (-1, 0, 1.0),
    (0, 1, 1.0),
    (0, -1, 1.0),
    (1, 1, std::f32::consts::SQRT_2),
    (1, -1, std::f32::consts::SQRT_2),
    (-1, 1, std::f32::consts::SQRT_2),
    (-1, -1, std::f32::consts::SQRT_2),
];

impl NavigationGraph {
    /// Flood out from the span `start` snaps to, entering only the areas in
    /// `mask`, as far as `max_length` metres of travel cost. `None` when `start`
    /// is off the navmesh.
    pub fn flood(&self, start: Vec3, max_length: f32, mask: u32) -> Option<Flood> {
        let from = self.snap(start)?;
        let spacing = self.grid_spacing.max(f32::EPSILON);
        let bound = (max_length / spacing).max(0.0);
        let mut cost: Map<u32, f32> = Map::default();
        let mut open = BinaryHeap::new();
        cost.insert(from.index, 0.0);
        open.push(Open { node: from, g: 0.0 });
        while let Some(here) = open.pop() {
            if cost.get(&here.node.index).is_some_and(|&g| here.g > g) {
                continue; // a stale heap entry
            }
            for (next, g) in self.moves(here, mask) {
                let best = cost.get(&next.index).copied().unwrap_or(f32::INFINITY);
                if g <= bound && g < best {
                    cost.insert(next.index, g);
                    open.push(Open { node: next, g });
                }
            }
        }
        Some(Flood { cost, spacing })
    }

    /// Every move out of `here` with the cost it reaches its span at: A\*'s
    /// `expand`, step for step.
    fn moves(&self, here: Open, mask: u32) -> Vec<(SpanRef, f32)> {
        let at = here.node;
        let NavSpan { y, area, .. } = self.spans[at.index as usize];
        let step = |dx: i32, dz: i32| {
            let to = self.link_to(at, at.gx + dx, at.gz + dz)?;
            in_mask(mask, self.spans[to.index as usize].area).then_some(to)
        };
        let (east, west, north, south) = (step(1, 0), step(-1, 0), step(0, 1), step(0, -1));
        let cost_of = |s: SpanRef| self.area_cost(self.spans[s.index as usize].area);
        let mut out = Vec::with_capacity(8);
        for &(dx, dz, horiz) in &DIRS {
            let (next, corner) = match (dx, dz) {
                (1, 0) => (east, MIN_AREA_COST),
                (-1, 0) => (west, MIN_AREA_COST),
                (0, 1) => (north, MIN_AREA_COST),
                (0, -1) => (south, MIN_AREA_COST),
                _ => {
                    let x = if dx > 0 { east } else { west };
                    let z = if dz > 0 { north } else { south };
                    let (Some(x), Some(z)) = (x, z) else {
                        continue;
                    };
                    (step(dx, dz), cost_of(x).max(cost_of(z)))
                }
            };
            let Some(next) = next else {
                continue;
            };
            let to = self.spans[next.index as usize];
            let cost = (0.5 * (self.area_cost(area) + cost_of(next))).max(corner);
            out.push((next, here.g + (horiz + (to.y - y).abs()) * cost));
        }
        for link in self.link_moves(at) {
            let lands = self.spans[link.to.index as usize].area;
            if in_mask(mask, link.area) && in_mask(mask, lands) {
                out.push((link.to, here.g + link.cost));
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "flood_tests.rs"]
mod flood_tests;
