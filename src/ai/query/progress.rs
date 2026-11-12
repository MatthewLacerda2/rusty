//! A query part-way through (#2116): generated and measured up to its first
//! `path` test, waiting for that test's search.
//!
//! As in Unreal's EQS, generation and the cheap tests run against the world when
//! the query starts ([`Progress::start`]); a `path` test needs only the navmesh
//! from then on ([`FloodAsk`]), so its search can run now ([`super::run`]) or be
//! stepped across ticks (`queue`). The navmesh tests are the dearest
//! ([`Measure::cost`](super::Measure::cost)), so they come last whatever the
//! spec's order, and once the first one is reached every test left is one.

use glam::Vec3;

use super::spec::{Anchor, Measure, QuerySpec};
use super::walk::FloodAsk;
use super::{generate, measure, rank, Item, QueryWorld};

/// A query between its tests: what is left of its items and the raw values the
/// tests run so far measured.
pub struct Progress {
    spec: QuerySpec,
    items: Vec<Vec3>,
    alive: Vec<usize>,
    raws: Vec<Vec<f32>>,
    /// The `path` tests still to run, in run order (spec order: one cost).
    path_tests: Vec<usize>,
    /// The search the head of `path_tests` waits for; `None` once none is left.
    ask: Option<FloodAsk>,
}

impl Progress {
    /// Generate `spec`'s items and run every test but the `path` ones, against
    /// `world` now (cheap first, written order within a cost).
    pub fn start(world: &QueryWorld, querier: Anchor, spec: QuerySpec) -> Self {
        let items = generate::items(world.nav, &spec.generator, spec.project);
        let mut order: Vec<usize> = (0..spec.tests.len()).collect();
        order.sort_by_key(|&t| spec.tests[t].measure.cost());
        let (path_tests, now): (Vec<usize>, Vec<usize>) = order
            .into_iter()
            .partition(|&t| is_path(&spec.tests[t].measure));
        let mut p = Self {
            alive: (0..items.len()).collect(),
            raws: vec![Vec::new(); spec.tests.len()],
            items,
            spec,
            path_tests,
            ask: None,
        };
        for t in now {
            let points = p.points();
            let values = measure::raw(world, querier, &p.spec.tests[t].measure, &points);
            p.apply(t, values);
        }
        p.next_ask(world, querier);
        p
    }

    /// The search the next `path` test waits for; `None` when the query is ready
    /// to rank.
    pub fn ask(&self) -> Option<&FloodAsk> {
        self.ask.as_ref()
    }

    /// Hand the waiting `path` test its search's lengths (one per item of its
    /// [`Self::ask`], in order) and run it; the next one, if any, asks from the
    /// same querier over what survived.
    pub fn answer(&mut self, lengths: Vec<Option<f32>>) {
        let (Some(ask), Some(&t)) = (self.ask.take(), self.path_tests.first()) else {
            return;
        };
        self.path_tests.remove(0);
        let values = match self.spec.tests[t].measure {
            Measure::PathExists => lengths
                .into_iter()
                .map(|l| Some(measure::answer(l.is_some())))
                .collect(),
            _ => lengths,
        };
        self.apply(t, values);
        self.ask = (!self.path_tests.is_empty())
            .then(|| FloodAsk::with_mask(ask.from, &self.points(), ask.mask))
            .flatten();
    }

    /// Every surviving item, best first: [`super::run`]'s answer. Call it once
    /// [`Self::ask`] is `None`.
    pub fn ranked(&self) -> Vec<Item> {
        rank::rank(&self.spec, &self.items, &self.alive, &self.raws)
    }

    /// The surviving items' positions.
    fn points(&self) -> Vec<Vec3> {
        self.alive.iter().map(|&i| self.items[i]).collect()
    }

    /// The search the head `path` test needs over the survivors, read now.
    fn next_ask(&mut self, world: &QueryWorld, querier: Anchor) {
        let points = self.points();
        self.ask = if self.path_tests.is_empty() {
            None
        } else {
            FloodAsk::new(world, querier, &points)
        };
        if self.ask.is_none() {
            self.path_tests.clear(); // nothing left to walk to: every test is done
        }
    }

    /// Record test `t`'s raw values for the survivors, in order, and keep the
    /// ones its filter passes. `None` fails the test outright (an unreachable
    /// item's path length).
    fn apply(&mut self, t: usize, values: Vec<Option<f32>>) {
        let test = &self.spec.tests[t];
        let mut row = vec![f32::NAN; self.items.len()];
        let mut kept = Vec::with_capacity(self.alive.len());
        for (&i, value) in self.alive.iter().zip(values) {
            let Some(v) = value else { continue };
            if test.filter.is_none_or(|f| f.keeps(v)) {
                row[i] = v;
                kept.push(i);
            }
        }
        self.raws[t] = row;
        self.alive = kept;
    }
}

fn is_path(m: &Measure) -> bool {
    matches!(m, Measure::PathExists | Measure::PathLength)
}
