//! crates/navigation/src/links.rs — which span in a neighbouring cell a move reaches (#454).
//!
//! Connectivity is a rule evaluated on demand rather than a stored edge list:
//! columns hold a handful of spans, so finding the one a step lands on is a
//! short scan, and nothing has to be rebuilt when the bake changes a cell. A
//! move from span `a` to span `b` is allowed when
//!
//! * the floor delta clears `max_step`, an absolute curb height (Recast's
//!   `walkableClimb`), and
//! * the open gap the two columns share, from the higher floor to the lower
//!   ceiling, still fits `agent_height` (Recast's `walkableHeight` rule), so an
//!   agent can't squeeze under a low lintel between two otherwise open spans.
//!
//! `max_slope` is not part of the rule. It decides which triangles are walkable at
//! rasterisation (`bake::raster`), as Recast's `walkableSlopeAngle` does; a ramp too
//! steep to walk bakes no spans to link. Held between neighbouring cell tops, a grade
//! would refuse any step taller than `max_slope × grid_spacing`, so a 0.15 m riser
//! joined at a 0.15 m grid and split at 0.1 (#2001): a finer grid disconnected what
//! a coarser one joined.
//!
//! When several spans of the target column qualify (rare: they'd need to sit
//! within a step of each other with a full agent height between them), the one
//! closest in height wins, then the lower one, so the answer is deterministic.

use super::{NavigationGraph, SpanRef};

impl NavigationGraph {
    /// The span in cell `(gx, gz)` an agent on `from` reaches by walking there in a
    /// straight line, or `None` when no span of that column is within a step.
    /// `(gx, gz)` may be any cell; only the floor delta and the shared headroom count,
    /// not the horizontal run between the cells.
    pub fn link_to(&self, from: SpanRef, gx: i32, gz: i32) -> Option<SpanRef> {
        if (gx, gz) == (from.gx, from.gz) {
            return Some(from);
        }
        let a = self.spans[from.index as usize];
        let mut best: Option<(f32, usize)> = None;
        for i in self.span_range(gx, gz) {
            let b = self.spans[i];
            if !self.steps(a, b) {
                continue;
            }
            let dh = (b.y - a.y).abs();
            if best.is_none_or(|(d, _)| dh < d) {
                best = Some((dh, i));
            }
        }
        best.map(|(_, i)| SpanRef {
            gx,
            gz,
            index: i as u32,
        })
    }

    /// Whether an agent can walk between spans `a` and `b` of neighbouring cells: the
    /// floor delta clears `max_step`, and the gap they share fits `agent_height`.
    /// Symmetric in `a` and `b`.
    pub(super) fn steps(&self, a: super::NavSpan, b: super::NavSpan) -> bool {
        let dh = (b.y - a.y).abs();
        let gap = a.ceiling.min(b.ceiling) - a.y.max(b.y);
        dh <= self.max_step && gap >= self.agent_height
    }

    /// The span an 8-way grid step `(dx, dz)` from `from` lands on. A diagonal step
    /// also needs both cardinal cells it cuts past to be reachable from `from`, so
    /// an agent never slips diagonally around a wall corner or a ledge.
    pub fn neighbour(&self, from: SpanRef, dx: i32, dz: i32) -> Option<SpanRef> {
        if dx != 0 && dz != 0 {
            self.link_to(from, from.gx + dx, from.gz)?;
            self.link_to(from, from.gx, from.gz + dz)?;
        }
        self.link_to(from, from.gx + dx, from.gz + dz)
    }
}

#[cfg(test)]
mod tests {
    use super::super::NavSpan;
    use super::*;

    /// A 3x1 strip whose middle cell holds a ground span and a second floor.
    fn stacked() -> NavigationGraph {
        let mut g = NavigationGraph::new(0.0, 2.0, 0.0, 0.0, 1.0);
        let ground = NavSpan {
            y: 0.0,
            ceiling: 3.0,
            area: 0,
            surface: crate::Surface::FLAT,
        };
        let upper = NavSpan {
            y: 3.2,
            ceiling: f32::INFINITY,
            area: 0,
            surface: crate::Surface::FLAT,
        };
        g.spans = vec![ground, ground, upper, ground];
        g.cell_start = vec![0, 1, 3, 4];
        g
    }

    fn at(gx: i32, index: u32) -> SpanRef {
        SpanRef { gx, gz: 0, index }
    }

    #[test]
    fn ground_links_to_ground_not_the_floor_above() {
        let g = stacked();
        assert_eq!(g.link_to(at(0, 0), 1, 0), Some(at(1, 1)));
        assert_eq!(g.link_to(at(1, 2), 2, 0), None, "3.2 m drop is no step");
    }

    #[test]
    fn low_shared_gap_blocks_the_move() {
        let mut g = stacked();
        g.agent_height = 3.5; // more than the 3.0 under the upper floor
        assert_eq!(g.link_to(at(0, 0), 1, 0), None);
    }

    #[test]
    fn only_max_step_limits_a_step() {
        // A 0.15 m step on a 0.1 m grid is a 1.5 grade between cell centres: the
        // slope limit is the rasteriser's, so the step still links (#2001).
        let mut g = NavigationGraph::new(0.0, 0.1, 0.0, 0.0, 0.1);
        g.spans[1].y = 0.15;
        let from = at(0, 0);
        assert!(g.link_to(from, 1, 0).is_some(), "0.15 is within max_step");
        g.spans[1].y = g.max_step;
        assert!(g.link_to(from, 1, 0).is_some(), "exactly max_step");
        g.spans[1].y = g.max_step + 0.01;
        assert!(g.link_to(from, 1, 0).is_none(), "past max_step");
    }

    #[test]
    fn diagonal_needs_both_cardinals() {
        let mut g = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 1.0);
        let from = SpanRef {
            gx: 0,
            gz: 0,
            index: 0,
        };
        assert!(g.neighbour(from, 1, 1).is_some());
        g.spans[1].y = 3.0; // cell (1, 0) becomes a wall top
        assert!(g.neighbour(from, 1, 1).is_none(), "no corner cutting");
    }
}
