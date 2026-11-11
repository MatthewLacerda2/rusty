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
//! is shrunk by [`Goals::slack`], `4 ε (bound + 4)`: a path of total cost `T ≤
//! bound` whose moves each cost at least one cell rounds by at most `ε T` per
//! cell still ahead, and `h` gives up `ε (bound + 4)` per cell, four times over,
//! which also covers rounding in `h` and in the key's own sum. With the slack,
//! every span on the cheapest path keys at or under its goal's cost, so each
//! goal settles at the very `f32` the full Dijkstra flood gives it.
//!
//! **A settled goal raises `h`** for the frontier left, so keys already in the
//! heap are lower than a fresh one would be. They are not re-keyed: a key from a
//! larger goal set is still a lower bound to every goal left, which is all the
//! argument above needs, so each settled cost stays exact. A stale key only
//! makes its span pop sooner than needed.

use crate::navigation::astar::heuristic;
use crate::navigation::{NavigationGraph, SpanRef};

pub(super) struct Goals {
    /// The goals not settled yet, deduplicated.
    left: Vec<SpanRef>,
    /// Whether there were any: a flood with none runs to its bound.
    any: bool,
    /// What `h` keeps of the octile distance: `1 - slack`, or 0 for none.
    keep: f32,
}

impl Goals {
    /// The goals `points` snap to, less those no walk or link chain leads to
    /// from `from` (`request::islands`): they are out of reach for any mask,
    /// so the flood need not run to its bound to find that out.
    pub(super) fn new(
        nav: &NavigationGraph,
        from: SpanRef,
        points: &[glam::Vec3],
        bound: f32,
    ) -> Self {
        let mut left: Vec<SpanRef> = points.iter().filter_map(|&p| nav.snap(p)).collect();
        left.sort_unstable_by_key(|s| s.index);
        left.dedup_by_key(|s| s.index);
        let any = !left.is_empty();
        nav.retain_reachable(from, &mut left);
        Self {
            left,
            any,
            keep: (1.0 - Self::slack(bound)).max(0.0),
        }
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
