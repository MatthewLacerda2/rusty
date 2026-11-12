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
//! the next bake (`cache`).

use std::collections::BinaryHeap;

use glam::Vec3;

use super::pool::NodePool;
use super::NavigationGraph;
use goals::Goals;
use moves::{Move, Open};

pub(super) use cache::FloodCache;

mod cache;
mod goals;
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
    /// navmesh is ignored; with none at all, it is [`Self::flood`].
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
        let mut goals = Goals::new(self, from, goals, bound);
        let mut cost = self.search.take(self.spans.len());
        let mut reached = 1;
        let mut open = BinaryHeap::new();
        let mut moves: Vec<Move> = Vec::with_capacity(8);
        cost.set(from.index, 0.0, None);
        open.push(Open::new(from, 0.0, goals.h(from)));
        if goals.done() {
            open.clear(); // every goal is out of reach
        }
        self.flood_cache.with(self, |cache| {
            while let Some(here) = open.pop() {
                let (node, g) = (here.node(), here.g());
                if g > cost.g(node.index) {
                    continue; // a stale heap entry
                }
                goals.settle(node.index);
                if goals.done() {
                    break;
                }
                moves.clear();
                self.moves(here, mask, cache.has_exit(node.index), &mut moves);
                for &(next, g) in &moves {
                    let best = cost.g(next.index);
                    if g >= best || g > bound {
                        continue;
                    }
                    let f = g + goals.h(next);
                    if f <= bound {
                        reached += usize::from(best == f32::INFINITY);
                        cost.set(next.index, g, None);
                        open.push(Open::new(next, g, f));
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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod island_tests;

#[cfg(test)]
mod order_tests;

// The bench level it runs on is a `dev` test fixture.
#[cfg(all(test, feature = "dev"))]
mod exact_tests;
