//! The goals a flood heads for, and the heuristic that heads it there (#2088).
//!
//! `h(n)` is the least octile distance, in cells, from `n` to a goal not yet
//! settled. Every move costs at least its octile distance (a step is its run
//! times an area cost of at least [`MIN_AREA_COST`](crate::navigation::MIN_AREA_COST),
//! a link never below the octile distance it spans, `offmesh`), so `h` never
//! overestimates and each goal settles at its exact cost.
//!
//! **Bit-exact, not only exact.** The flood's costs are `f32` sums, and a sum's
//! rounding can make the cheapest path's cost an ulp *under* the real-valued
//! `g + h` of a span on it, which would let a goal pop an ulp too dear. So `h`
//! is shrunk by [`Goals::slack`], `4 ε (bound + 4)`. A path of total cost `T ≤
//! bound` rounds by at most `ε T` per move still ahead. A move that closes at
//! least one cell of octile distance (every grid step, and every link whose ends
//! lie in different grid columns) is paid for by `h` giving up `ε (bound + 4)`
//! per cell, four times over, which also covers rounding in `h` and in the key's
//! own sum. A move that closes none (an authored link whose ends share a column,
//! a ladder: octile 0, its cost its `cost_override`) is paid for by itself: `h`
//! counts none of its cost `c`, and adding `c` to an `f32` rounds by at most `c`,
//! as the sum before it is itself an `f32`. With the slack, every span on the
//! cheapest path keys at or under its goal's cost, so each goal settles at the
//! very `f32` the full Dijkstra flood gives it (`exact_tests` holds it, a ladder
//! included).
//!
//! **A settled goal raises `h`** for the frontier left, so keys already in the
//! heap are lower than a fresh one would be. They are not re-keyed: a key from a
//! larger goal set is still a lower bound to every goal left, which is all the
//! argument above needs, so each settled cost stays exact. A stale key only
//! makes its span pop sooner than needed.

use crate::navigation::astar::heuristic;
use glam::Vec3;

use crate::navigation::{NavigationGraph, SpanRef};

#[derive(Default)]
pub(super) struct Goals {
    /// The goals not settled yet, deduplicated.
    left: Vec<SpanRef>,
    /// Whether there were any: a flood with none runs to its bound.
    any: bool,
    /// What `h` keeps of the octile distance: `1 - slack`, or 0 for none.
    keep: f32,
    /// Every span the goal points snapped to, and those the island map kept: a
    /// flood carried across a rebake must find the very same ones (`carry`).
    all: Vec<SpanRef>,
    kept: Vec<SpanRef>,
}

impl Goals {
    /// The goals `points` snap to, less those no walk or link chain leads to
    /// from `from` (`request::islands`): they are out of reach for any mask,
    /// so the flood need not run to its bound to find that out.
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

    /// Renumber the goals onto a rebaked navmesh by `remap` (`carry`): `false`,
    /// changing nothing, when `remap` refuses one, or when `points` no longer
    /// snap to the same spans or the island map from `from` keeps others.
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

    /// The share of the octile distance `h` gives up for `f32` rounding (module doc).
    pub(super) fn slack(bound: f32) -> f32 {
        4.0 * f32::EPSILON * (bound + 4.0)
    }

    /// The least cost a path from `n` to a goal still unsettled could have:
    /// 0 with no goals (a plain Dijkstra flood).
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

    /// Mark the goal at span `index`, if there is one, settled.
    pub(super) fn settle(&mut self, index: u32) {
        if let Some(i) = self.left.iter().position(|g| g.index == index) {
            self.left.remove(i);
        }
    }

    /// Whether the flood has what it came for: it had goals, and every one is
    /// settled or proven out of reach.
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
