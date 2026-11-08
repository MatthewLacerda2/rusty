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
use crate::core::collections::{Map, Set};

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
        self.flood_to(start, &[], max_length, mask)
    }

    /// [`Self::flood`] that stops early once the span each of `goals` snaps to is
    /// settled (its cost final), instead of covering the whole bound: the
    /// environment query asks about a few dozen spots and the flood around them
    /// is most of its cost. Exact for the goals; another span it reports may
    /// hold a cost a longer flood would still lower. A goal off the navmesh is
    /// ignored; with none left, it runs to the bound.
    pub fn flood_to(
        &self,
        start: Vec3,
        goals: &[Vec3],
        max_length: f32,
        mask: u32,
    ) -> Option<Flood> {
        let from = self.snap(start)?;
        let spacing = self.grid_spacing.max(f32::EPSILON);
        let bound = (max_length / spacing).max(0.0);
        // Each goal with the cost past which it is overdue: its flat distance
        // in cells (the least a path to it could cost), stretched as a path
        // round a wall or two would be, so a goal on the start's own floor
        // settles before it is ever checked.
        let mut overdue: Vec<(f32, SpanRef)> = goals
            .iter()
            .filter_map(|&g| self.snap(g))
            .map(|s| {
                let least = self.run_between(from, s.gx, s.gz) / spacing;
                (least * OVERDUE_STRETCH + OVERDUE_CELLS, s)
            })
            .collect();
        overdue.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.index.cmp(&b.1.index)));
        let mut waiting: Set<u32> = overdue.iter().map(|(_, s)| s.index).collect();
        let stop_early = !waiting.is_empty();
        let mut islands = Islands::new(self);
        let mut next_overdue = 0;
        let mut cost: Map<u32, f32> = Map::default();
        let mut open = BinaryHeap::new();
        cost.insert(from.index, 0.0);
        open.push(Open { node: from, g: 0.0 });
        while let Some(here) = open.pop() {
            if cost.get(&here.node.index).is_some_and(|&g| here.g > g) {
                continue; // a stale heap entry
            }
            waiting.remove(&here.node.index);
            // A goal still unsettled past its least possible cost may sit on an
            // island the flood can never reach: check once, and drop it if so.
            while let Some(&(least, goal)) = overdue.get(next_overdue) {
                if least >= here.g {
                    break;
                }
                next_overdue += 1;
                if waiting.contains(&goal.index) && islands.cut_off(goal, from) {
                    waiting.remove(&goal.index);
                }
            }
            if stop_early && waiting.is_empty() {
                break; // every goal settled, or proven out of reach
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

/// When a goal counts as overdue: its least possible cost times this, plus
/// [`OVERDUE_CELLS`].
const OVERDUE_STRETCH: f32 = 1.5;
const OVERDUE_CELLS: f32 = 4.0;

/// The most spans an island check visits before it calls a goal's piece of the
/// navmesh "too big to be an island" and leaves it to the flood. A crate top or
/// a roof is a few hundred spans at most.
const ISLAND_SPANS: usize = 512;

/// The island check: whether a goal's piece of navmesh is cut off from the start.
/// It walks out from the goal over every connection in **either** direction
/// (every span of a neighbouring column within a step, no corner rule, every
/// off-mesh link both ways, no area mask): a superset of what can reach the
/// goal, so a closed piece without the start proves the goal unreachable.
struct Islands<'a> {
    nav: &'a NavigationGraph,
    /// Off-mesh link ends as `(span, other end's span)`, both ways, sorted; built
    /// the first time a check runs.
    links: Option<Vec<(u32, SpanRef)>>,
    /// Spans already proven cut off.
    cut: Set<u32>,
}

impl<'a> Islands<'a> {
    fn new(nav: &'a NavigationGraph) -> Self {
        Self {
            nav,
            links: None,
            cut: Set::default(),
        }
    }

    /// Whether `goal` lies on a piece of at most [`ISLAND_SPANS`] spans that
    /// does not hold `start`.
    fn cut_off(&mut self, goal: SpanRef, start: SpanRef) -> bool {
        if self.cut.contains(&goal.index) {
            return true;
        }
        let nav = self.nav;
        let links = self.links.get_or_insert_with(|| link_ends(nav));
        let mut seen: Set<u32> = Set::default();
        seen.insert(goal.index);
        let mut stack = vec![goal];
        while let Some(s) = stack.pop() {
            if s.index == start.index {
                return false;
            }
            let a = nav.spans[s.index as usize];
            let first = links.partition_point(|l| l.0 < s.index);
            let jumps = links[first..].iter().take_while(|l| l.0 == s.index);
            let mut next: Vec<SpanRef> = jumps.map(|l| l.1).collect();
            for &(dx, dz, _) in &DIRS {
                let (gx, gz) = (s.gx + dx, s.gz + dz);
                let run = nav.run_between(s, gx, gz);
                for i in nav.span_range(gx, gz) {
                    if nav.steps(a, nav.spans[i], run) {
                        next.push(SpanRef {
                            gx,
                            gz,
                            index: i as u32,
                        });
                    }
                }
            }
            for n in next {
                if seen.insert(n.index) {
                    if seen.len() > ISLAND_SPANS {
                        return false;
                    }
                    stack.push(n);
                }
            }
        }
        self.cut.extend(seen);
        true
    }
}

/// Every off-mesh link's two end spans, each way round, sorted by the first.
fn link_ends(nav: &NavigationGraph) -> Vec<(u32, SpanRef)> {
    let mut out = Vec::new();
    for link in nav.offmesh_links() {
        if let (Some(a), Some(b)) = (nav.link_span(link.from), nav.link_span(link.to)) {
            out.push((a.index, b));
            out.push((b.index, a));
        }
    }
    out.sort_unstable_by_key(|l| (l.0, l.1.index));
    out
}

#[cfg(test)]
#[path = "flood_tests.rs"]
mod flood_tests;
