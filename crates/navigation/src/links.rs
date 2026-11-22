//! A move needs the floor delta within `max_step` (Recast's `walkableClimb`) and the shared gap to
//! fit `agent_height` (`walkableHeight`). `max_slope` is deliberately not part of it: held between
//! cell tops it would refuse steps taller than `max_slope × grid_spacing`, so a finer grid
//! disconnected what a coarser one joined. Of several qualifying spans the closest in height wins,
//! then the lower.

use super::{NavigationGraph, SpanRef};

impl NavigationGraph {
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

    pub(super) fn steps(&self, a: super::NavSpan, b: super::NavSpan) -> bool {
        let dh = (b.y - a.y).abs();
        let gap = a.ceiling.min(b.ceiling) - a.y.max(b.y);
        dh <= self.max_step && gap >= self.agent_height
    }

    /// A diagonal also needs both cardinal cells it cuts past, so an agent never slips around a
    /// corner.
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
        g.agent_height = 3.5;
        assert_eq!(g.link_to(at(0, 0), 1, 0), None);
    }

    #[test]
    fn only_max_step_limits_a_step() {
        // A 0.15 m step on a 0.1 m grid: the slope limit is the rasteriser's, so the step still
        // links.
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
        g.spans[1].y = 3.0;
        assert!(g.neighbour(from, 1, 1).is_none(), "no corner cutting");
    }
}
