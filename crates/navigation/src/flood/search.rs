//! The synchronous flood lends the search the graph's shared pool; a held search keeps a pool of its
//! own, so a script's synchronous query in between never touches its slots. After an incremental
//! rebake that reaches nothing it holds it is carried (`carry`); after any other rebake (a full bake,
//! or an area-cost change) it starts over, since its costs may no longer hold.

use std::collections::BinaryHeap;

use glam::Vec3;

use super::goals::Goals;
use super::moves::{Move, Open};
use crate::pool::NodePool;
use crate::{NavigationGraph, SpanRef};

#[derive(Clone, Default)]
pub(super) struct Ask {
    start: Vec3,
    pub(super) goals: Vec<Vec3>,
    max_length: f32,
    mask: u32,
}

#[derive(Default)]
pub struct FloodSearch {
    pub(super) cost: NodePool,
    pub(super) open: BinaryHeap<Open>,
    moves: Vec<Move>,
    pub(super) ask: Ask,
    pub(super) goals: Goals,
    pub(super) from: Option<SpanRef>,
    bound: f32,
    pub(super) spacing: f32,
    pub(super) reached: usize,
    pub(super) key: (u64, usize),
    pub(super) cells: Vec<u32>,
    pub(super) cells_fresh: bool,
    pub(super) done: bool,
}

impl FloodSearch {
    pub(super) fn with_pool(cost: NodePool) -> Self {
        Self {
            cost,
            ..Self::default()
        }
    }

    pub fn length_to(&self, nav: &NavigationGraph, p: Vec3) -> Option<f32> {
        let span = nav.snap(p)?;
        let current = self.key == (nav.bake_generation, nav.spans.len());
        let g = (current && self.cost.covers(span.index)).then(|| self.cost.g(span.index))?;
        g.is_finite().then_some(g * self.spacing)
    }
}

impl NavigationGraph {
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

    fn restart_flood(&self, s: &mut FloodSearch) {
        s.key = (self.bake_generation, self.spans.len());
        s.cells_fresh = false;
        s.cost.begin(self.spans.len());
        s.open.clear();
        s.reached = 0;
        s.spacing = self.grid_spacing.max(f32::EPSILON);
        s.bound = (s.ask.max_length / s.spacing).max(0.0);
        s.from = self.snap(s.ask.start);
        let Some(from) = s.from else {
            (s.done, s.goals) = (true, Goals::default());
            return;
        };
        s.goals = Goals::new(self, from, &s.ask.goals, s.bound);
        (s.done, s.reached) = (false, 1);
        s.cost.set(from.index, 0.0, None);
        if !s.goals.done() {
            s.open.push(Open::new(from, 0.0, s.goals.h(from)));
        }
    }

    pub fn step_flood(&self, search: &mut FloodSearch, budget: &mut u32) -> bool {
        if search.key != (self.bake_generation, self.spans.len()) && !self.carry_flood(search) {
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
                    continue;
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
        if !s.done && !s.cells_fresh {
            s.cells.clone_from(&self.cell_start);
            s.cells_fresh = true;
        }
        s.done
    }
}
