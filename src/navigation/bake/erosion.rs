//! src/navigation/bake/erosion.rs — pull the walkable spans back by the agent radius (#277).
//!
//! An agent is a disc of `agent_radius`, so a span whose centre is within that radius
//! of an edge of the walkable surface can't hold its footprint. Recast's
//! `rcErodeWalkableArea`, on spans:
//!
//! * A **boundary** span is missing a link (`links.rs`) to at least one of its four
//!   cardinal neighbours: a wall, a ledge, a hole, a low-headroom gap or the world
//!   edge is next to it. A stair or ramp links all the way through, so erosion never
//!   cuts across one.
//! * A chamfer distance (2 per cardinal move, 3 per diagonal, through the same links)
//!   spreads from the boundary spans, and every span closer than `2 * radius_cells`
//!   is dropped. `radius_cells = ceil(agent_radius / grid_spacing)` rounds *up*, so a
//!   partly covered cell still erodes and no agent clips a wall.
//! * Distances come from the pre-erosion surface, so it is one shrink by the radius,
//!   not an iterated one. `radius_cells == 0` is an exact no-op.
//!
//! Deterministic: a Dijkstra with integer costs and a total order on its queue.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::super::{NavigationGraph, SpanRef};

const CARDINALS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const DIAGONALS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

impl NavigationGraph {
    /// Drop every span within `agent_radius` of the walkable surface's edge.
    pub(super) fn erode_for_agent_radius(&mut self, agent_radius: f32) {
        let radius_cells = (agent_radius / self.grid_spacing).ceil() as i32;
        if radius_cells <= 0 {
            return;
        }
        let threshold = 2 * radius_cells as u32;
        let mut dist = vec![u32::MAX; self.spans.len()];
        let mut queue = BinaryHeap::new();
        for s in self.span_refs() {
            if CARDINALS
                .iter()
                .any(|&(dx, dz)| self.neighbour(s, dx, dz).is_none())
            {
                dist[s.index as usize] = 0;
                queue.push(Reverse((0, s.index, s.gx, s.gz)));
            }
        }
        while let Some(Reverse((d, index, gx, gz))) = queue.pop() {
            if d > dist[index as usize] {
                continue;
            }
            let here = SpanRef { gx, gz, index };
            let steps = CARDINALS
                .iter()
                .map(|&m| (m, 2))
                .chain(DIAGONALS.iter().map(|&m| (m, 3)));
            for ((dx, dz), cost) in steps {
                let Some(n) = self.neighbour(here, dx, dz) else {
                    continue;
                };
                let nd = d + cost;
                if nd < threshold && nd < dist[n.index as usize] {
                    dist[n.index as usize] = nd;
                    queue.push(Reverse((nd, n.index, n.gx, n.gz)));
                }
            }
        }
        let keep: Vec<bool> = dist.iter().map(|&d| d >= threshold).collect();
        self.retain_spans(&keep);
    }

    /// Keep only the spans whose `keep` flag is set, re-deriving the cell ranges.
    pub(super) fn retain_spans(&mut self, keep: &[bool]) {
        let mut kept = 0u32;
        let mut cell_start = Vec::with_capacity(self.cell_start.len());
        for w in self.cell_start.windows(2) {
            cell_start.push(kept);
            kept += keep[w[0] as usize..w[1] as usize]
                .iter()
                .filter(|&&k| k)
                .count() as u32;
        }
        cell_start.push(kept);
        let mut i = 0;
        self.spans.retain(|_| {
            i += 1;
            keep[i - 1]
        });
        self.cell_start = cell_start;
    }
}
