//! src/navigation/links.rs — which span in a neighbouring cell a move reaches (#454).
//!
//! Connectivity is a rule evaluated on demand rather than a stored edge list:
//! columns hold a handful of spans, so finding the one a step lands on is a
//! short scan, and nothing has to be rebuilt when the bake changes a cell. A
//! move from span `a` to span `b` is allowed when
//!
//! * the floor delta clears both `max_step` (an absolute curb height) and
//!   `max_slope` (rise per unit of horizontal travel, so a diagonal tolerates a
//!   slightly larger rise than a cardinal move), and
//! * the open gap the two columns share, from the higher floor to the lower
//!   ceiling, still fits `agent_height` (Recast's `walkableHeight` rule), so an
//!   agent can't squeeze under a low lintel between two otherwise open spans.
//!
//! When several spans of the target column qualify (rare: they'd need to sit
//! within a step of each other with a full agent height between them), the one
//! closest in height wins, then the lower one, so the answer is deterministic.

use super::{NavigationGraph, SpanRef};

impl NavigationGraph {
    /// The span in cell `(gx, gz)` an agent on `from` reaches by walking there in a
    /// straight line, or `None` when no span of that column is within a step.
    /// `(gx, gz)` may be any cell; the horizontal run is the centre-to-centre distance.
    pub fn link_to(&self, from: SpanRef, gx: i32, gz: i32) -> Option<SpanRef> {
        if (gx, gz) == (from.gx, from.gz) {
            return Some(from);
        }
        let a = self.spans[from.index as usize];
        let (dx, dz) = ((gx - from.gx) as f32, (gz - from.gz) as f32);
        let run = self.grid_spacing * (dx * dx + dz * dz).sqrt();
        let mut best: Option<(f32, usize)> = None;
        for i in self.span_range(gx, gz) {
            let b = self.spans[i];
            let dh = (b.y - a.y).abs();
            let gap = a.ceiling.min(b.ceiling) - a.y.max(b.y);
            if dh > self.max_step || dh > self.max_slope * run || gap < self.agent_height {
                continue;
            }
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
        };
        let upper = NavSpan {
            y: 3.2,
            ceiling: f32::INFINITY,
            area: 0,
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
    fn step_and_slope_limits_both_apply() {
        let mut g = NavigationGraph::new(0.0, 1.0, 0.0, 0.0, 1.0);
        g.spans[1].y = 0.5;
        let from = at(0, 0);
        assert!(g.link_to(from, 1, 0).is_some(), "0.5 is exactly max_step");
        g.max_slope = 0.4;
        assert!(
            g.link_to(from, 1, 0).is_none(),
            "0.5 rise over 1.0 run > 0.4"
        );
        g.max_slope = 1.0;
        g.max_step = 0.4;
        assert!(g.link_to(from, 1, 0).is_none(), "0.5 step > max_step 0.4");
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
