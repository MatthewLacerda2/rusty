//! `h(n)` is the least octile distance to an unsettled goal; every move costs at least its octile
//! distance, so `h` never overestimates. The flood's `f32` sums can round the cheapest path an ulp
//! *under* the real `g + h`, so `h` is shrunk by [`Goals::slack`], `4 ε (bound + 4)`: each goal then
//! settles at the very `f32` the full Dijkstra flood gives it. A settled goal raises `h`, but keys
//! already in the heap are not re-keyed: they stay lower bounds.

use crate::astar::heuristic;
use glam::Vec3;

use crate::{NavigationGraph, SpanRef};

#[derive(Default)]
pub(super) struct Goals {
    left: Vec<SpanRef>,
    any: bool,
    keep: f32,
    all: Vec<SpanRef>,
    kept: Vec<SpanRef>,
}

impl Goals {
    pub(super) fn new(nav: &NavigationGraph, from: SpanRef, points: &[Vec3], bound: f32) -> Self {
        let all = snapped(nav, points);
        let mut kept = all.clone();
        nav.retain_reachable(from, &mut kept);
        Self {
            left: kept.clone(),
            any: !all.is_empty(),
            keep: (1.0 - Self::slack(bound)).max(0.0),
            all,
            kept,
        }
    }

    pub(super) fn carry(
        &mut self,
        nav: &NavigationGraph,
        from: SpanRef,
        points: &[Vec3],
        remap: &dyn Fn(u32) -> Option<SpanRef>,
    ) -> bool {
        let map = |v: &[SpanRef]| v.iter().map(|g| remap(g.index)).collect::<Option<Vec<_>>>();
        let (Some(all), Some(kept), Some(left)) =
            (map(&self.all), map(&self.kept), map(&self.left))
        else {
            return false;
        };
        let mut reach = all.clone();
        nav.retain_reachable(from, &mut reach);
        if snapped(nav, points) != all || reach != kept {
            return false;
        }
        (self.all, self.kept, self.left) = (all, kept, left);
        true
    }

    pub(super) fn slack(bound: f32) -> f32 {
        4.0 * f32::EPSILON * (bound + 4.0)
    }

    pub(super) fn h(&self, n: SpanRef) -> f32 {
        let least = self
            .left
            .iter()
            .map(|&g| heuristic(n, g))
            .fold(f32::INFINITY, f32::min);
        if least.is_finite() {
            least * self.keep
        } else {
            0.0
        }
    }

    pub(super) fn settle(&mut self, index: u32) {
        if let Some(i) = self.left.iter().position(|g| g.index == index) {
            self.left.remove(i);
        }
    }

    pub(super) fn done(&self) -> bool {
        self.any && self.left.is_empty()
    }
}

/// The spans `points` snap to, by index, each once.
fn snapped(nav: &NavigationGraph, points: &[Vec3]) -> Vec<SpanRef> {
    let mut spans: Vec<SpanRef> = points.iter().filter_map(|&p| nav.snap(p)).collect();
    spans.sort_unstable_by_key(|s| s.index);
    spans.dedup_by_key(|s| s.index);
    spans
}
