//! src/navigation/flood/mod.rs — one bounded Dijkstra flood from a point (#1892).
//!
//! An environment query's `path` test asks, for every candidate spot, whether the
//! querier can walk there and how far. One A\* per item is what Unreal pays; a
//! single flood out from the querier answers every item at once. The flood walks
//! the same moves and costs as [`NavigationGraph::find_path_masked`] (8-way span
//! links, the diagonal corner rule, area costs, off-mesh links, the area mask with
//! the start span exempt), but with no goal and no heuristic, and stops at
//! `max_length`, so it costs the area it covers, not the level.
//!
//! Its costs live in the graph's stamped search pool (`pool`, #2083), as A\*'s do:
//! starting a flood costs nothing per span, and a cost is an array read, not a
//! hash (#2088). The [`Flood`] holds the pool while it is read and hands it back
//! when dropped. What depends on the navmesh alone (the island check's answers,
//! which spans a link leaves from) is kept on the graph until the next bake
//! (`cache`).

use std::collections::BinaryHeap;

use glam::Vec3;

use super::pool::NodePool;
use super::{NavigationGraph, SpanRef};
use moves::{Move, Open};

pub(super) use cache::FloodCache;

mod cache;
mod islands;
mod moves;

/// The spans a flood reached, each with its travel cost from the start. Holds
/// the graph's search pool until it is dropped: a search that runs meanwhile
/// works on a fresh one.
pub struct Flood<'a> {
    nav: &'a NavigationGraph,
    /// Cost from the start span, in A\*'s units (cells × area cost), by span index.
    cost: NodePool,
    /// How many spans the flood reached.
    reached: usize,
    /// The grid's cell size, metres: turns a cost into metres.
    spacing: f32,
}

impl Flood<'_> {
    /// The travel cost from the flood's start to the span `p` snaps to, metres
    /// (area costs weighed in, as the planner weighs them), or `None` when the
    /// flood never reached it: unreachable, or past its bound.
    pub fn length_to(&self, nav: &NavigationGraph, p: Vec3) -> Option<f32> {
        let span = nav.snap(p)?;
        let g = self.cost.g(span.index);
        g.is_finite().then_some(g * self.spacing)
    }

    /// How many spans the flood reached.
    pub fn reached(&self) -> usize {
        self.reached
    }
}

impl Drop for Flood<'_> {
    fn drop(&mut self) {
        self.nav.search.restore(std::mem::take(&mut self.cost));
    }
}

impl NavigationGraph {
    /// Flood out from the span `start` snaps to, entering only the areas in
    /// `mask`, as far as `max_length` metres of travel cost. `None` when `start`
    /// is off the navmesh.
    pub fn flood(&self, start: Vec3, max_length: f32, mask: u32) -> Option<Flood<'_>> {
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
    ) -> Option<Flood<'_>> {
        let from = self.snap(start)?;
        let spacing = self.grid_spacing.max(f32::EPSILON);
        let bound = (max_length / spacing).max(0.0);
        let mut goals = Goals::new(self, from, goals, spacing);
        let mut cost = self.search.take(self.spans.len());
        let mut reached = 1;
        let mut open = BinaryHeap::new();
        let mut moves: Vec<Move> = Vec::with_capacity(8);
        cost.set(from.index, 0.0, None);
        open.push(Open::new(from, 0.0));
        self.flood_cache.with(self, |cache| {
            while let Some(here) = open.pop() {
                let (node, g) = (here.node(), here.g());
                if g > cost.g(node.index) {
                    continue; // a stale heap entry
                }
                goals.settle(node.index);
                // A goal still unsettled past its least possible cost may sit on
                // an island the flood can never reach: check once, drop it if so.
                while let Some(goal) = goals.next_overdue(g) {
                    if cache.islands.cut_off(self, goal, from) {
                        goals.settle(goal.index);
                    }
                }
                if goals.all_settled() {
                    break; // every goal settled, or proven out of reach
                }
                moves.clear();
                self.moves(here, mask, cache.has_exit(node.index), &mut moves);
                for &(next, g) in &moves {
                    let best = cost.g(next.index);
                    if g <= bound && g < best {
                        reached += usize::from(best == f32::INFINITY);
                        cost.set(next.index, g, None);
                        open.push(Open::new(next, g));
                    }
                }
            }
        });
        Some(Flood {
            nav: self,
            cost,
            reached,
            spacing,
        })
    }
}

/// The goals a flood waits for: each one's span, whether it is settled, and the
/// cost past which an unsettled one is overdue for the island check.
struct Goals {
    /// Goal span indices, sorted and deduplicated, with their settled flags.
    spans: Vec<(u32, bool)>,
    /// Goals not yet settled; never reaches 0 when there were none to begin with.
    left: usize,
    /// `(overdue cost, goal)`, cheapest first, and the next one to check.
    overdue: Vec<(f32, SpanRef)>,
    next: usize,
}

impl Goals {
    fn new(nav: &NavigationGraph, from: SpanRef, goals: &[Vec3], spacing: f32) -> Self {
        // Each goal with the cost past which it is overdue: its flat distance
        // in cells (the least a path to it could cost), stretched as a path
        // round a wall or two would be, so a goal on the start's own floor
        // settles before it is ever checked.
        let mut overdue: Vec<(f32, SpanRef)> = goals
            .iter()
            .filter_map(|&g| nav.snap(g))
            .map(|s| {
                let least = nav.run_between(from, s.gx, s.gz) / spacing;
                (least * OVERDUE_STRETCH + OVERDUE_CELLS, s)
            })
            .collect();
        overdue.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.index.cmp(&b.1.index)));
        let mut spans: Vec<(u32, bool)> = overdue.iter().map(|(_, s)| (s.index, false)).collect();
        spans.sort_unstable();
        spans.dedup();
        let left = if spans.is_empty() {
            usize::MAX
        } else {
            spans.len()
        };
        Self {
            spans,
            left,
            overdue,
            next: 0,
        }
    }

    /// Mark the goal at span `index`, if there is one, settled.
    fn settle(&mut self, index: u32) {
        if let Ok(i) = self.spans.binary_search_by_key(&index, |s| s.0) {
            if !std::mem::replace(&mut self.spans[i].1, true) {
                self.left -= 1;
            }
        }
    }

    /// The next goal, still unsettled, whose overdue cost is below `g`.
    fn next_overdue(&mut self, g: f32) -> Option<SpanRef> {
        while let Some(&(least, goal)) = self.overdue.get(self.next) {
            if least >= g {
                return None;
            }
            self.next += 1;
            let i = self.spans.binary_search_by_key(&goal.index, |s| s.0);
            if i.is_ok_and(|i| !self.spans[i].1) {
                return Some(goal);
            }
        }
        None
    }

    fn all_settled(&self) -> bool {
        self.left == 0
    }
}

/// When a goal counts as overdue: its least possible cost times this, plus
/// [`OVERDUE_CELLS`].
const OVERDUE_STRETCH: f32 = 1.5;
const OVERDUE_CELLS: f32 = 4.0;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod island_tests;

#[cfg(test)]
mod order_tests;
