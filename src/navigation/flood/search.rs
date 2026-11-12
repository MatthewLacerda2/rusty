//! The flood as a resumable search (#2116): begun once, then stepped on a budget
//! of node expansions, so the environment query's time-sliced form can carry one
//! across ticks, as the agents' path queue carries an A\* (`request`).
//!
//! [`NavigationGraph::flood_to`] is this search stepped to its end in one call,
//! so a sliced flood and a synchronous one pop the same spans in the same order
//! and give every goal the very same `f32` cost. A search owns its pool, heap and
//! goals: the synchronous flood lends it the graph's shared pool for the call
//! and its answer (`Flood`); a held search keeps a pool of its own, so a script's
//! synchronous query in between never touches its slots.
//!
//! A search remembers what it was asked and the bake it runs on. Stepped after a
//! rebake (or an area-cost change, which bumps the same generation) it starts
//! over on the new navmesh from the same ask: its costs so far may no longer hold.

use std::collections::BinaryHeap;

use glam::Vec3;

use super::goals::Goals;
use super::moves::{Move, Open};
use crate::navigation::pool::NodePool;
use crate::navigation::NavigationGraph;

/// What a flood was asked: its start, goals, bound (metres) and area mask.
#[derive(Clone, Default)]
struct Ask {
    start: Vec3,
    goals: Vec<Vec3>,
    max_length: f32,
    mask: u32,
}

/// A flood in progress, or finished (see the module doc). A default one has
/// been asked nothing and reaches nothing.
#[derive(Default)]
pub struct FloodSearch {
    pub(super) cost: NodePool,
    open: BinaryHeap<Open>,
    moves: Vec<Move>,
    ask: Ask,
    goals: Goals,
    /// The bound in cells, and the cell size, metres.
    bound: f32,
    pub(super) spacing: f32,
    pub(super) reached: usize,
    /// The bake the costs are for: `(bake_generation, span count)`.
    key: (u64, usize),
    /// Whether the start snapped onto the navmesh.
    on_mesh: bool,
    done: bool,
}

impl FloodSearch {
    /// A search that floods with `pool` (the synchronous flood lends the graph's).
    pub(super) fn with_pool(cost: NodePool) -> Self {
        Self {
            cost,
            ..Self::default()
        }
    }

    /// Whether the search has finished: every goal settled or proven out of
    /// reach, or its bound reached.
    pub fn done(&self) -> bool {
        self.done
    }

    /// Whether its start was on the navmesh; off it, the search reaches nothing.
    pub fn on_mesh(&self) -> bool {
        self.on_mesh
    }

    /// The travel cost from the start to the span `p` snaps to, metres, or
    /// `None` when not reached (see [`super::Flood::length_to`]).
    pub fn length_to(&self, nav: &NavigationGraph, p: Vec3) -> Option<f32> {
        let span = nav.snap(p)?;
        let g = self.cost.g(span.index);
        g.is_finite().then_some(g * self.spacing)
    }
}

impl NavigationGraph {
    /// Begin `search` as [`Self::flood_to`] would, from `start` toward `goals`
    /// within `max_length` metres over the areas in `mask`, discarding what it
    /// held. Nothing is expanded yet: [`Self::step_flood`] runs it.
    pub fn begin_flood(
        &self,
        search: &mut FloodSearch,
        start: Vec3,
        goals: &[Vec3],
        max_length: f32,
        mask: u32,
    ) {
        search.ask = Ask {
            start,
            goals: goals.to_vec(),
            max_length,
            mask,
        };
        self.restart_flood(search);
    }

    /// Start `search` over from its ask on the navmesh as it is now.
    fn restart_flood(&self, s: &mut FloodSearch) {
        s.key = (self.bake_generation, self.spans.len());
        s.cost.begin(self.spans.len());
        s.open.clear();
        s.reached = 0;
        s.spacing = self.grid_spacing.max(f32::EPSILON);
        s.bound = (s.ask.max_length / s.spacing).max(0.0);
        let Some(from) = self.snap(s.ask.start) else {
            (s.on_mesh, s.done, s.goals) = (false, true, Goals::default());
            return;
        };
        s.goals = Goals::new(self, from, &s.ask.goals, s.bound);
        (s.on_mesh, s.done, s.reached) = (true, false, 1);
        s.cost.set(from.index, 0.0, None);
        if !s.goals.done() {
            s.open.push(Open::new(from, 0.0, s.goals.h(from)));
        } // else every goal is out of reach
    }

    /// Run `search` for at most `budget` node expansions, taking what it spends
    /// off `budget`: `true` once it has finished. A search begun on an earlier
    /// bake starts over first (module doc).
    pub fn step_flood(&self, search: &mut FloodSearch, budget: &mut u32) -> bool {
        if search.key != (self.bake_generation, self.spans.len()) {
            self.restart_flood(search);
        }
        let s = search;
        let mask = s.ask.mask;
        self.flood_cache.with(self, |cache| {
            while !s.done && *budget > 0 {
                let Some(here) = s.open.pop() else {
                    s.done = true;
                    break;
                };
                let (node, g) = (here.node(), here.g());
                if g > s.cost.g(node.index) {
                    continue; // a stale heap entry
                }
                *budget -= 1;
                s.goals.settle(node.index);
                if s.goals.done() {
                    s.done = true;
                    break;
                }
                s.moves.clear();
                self.moves(here, mask, cache.has_exit(node.index), &mut s.moves);
                for &(next, g) in &s.moves {
                    let best = s.cost.g(next.index);
                    if g >= best || g > s.bound {
                        continue;
                    }
                    let f = g + s.goals.h(next);
                    if f <= s.bound {
                        s.reached += usize::from(best == f32::INFINITY);
                        s.cost.set(next.index, g, None);
                        s.open.push(Open::new(next, g, f));
                    }
                }
            }
            if s.open.is_empty() {
                s.done = true;
            }
        });
        s.done
    }
}
