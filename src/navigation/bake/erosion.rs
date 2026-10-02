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
//! **Locality (#456).** A kept-or-dropped verdict needs a boundary span within
//! `radius_cells - 1` steps (every step costs at least 2), and a span's boundary test
//! looks one cell further. So the verdicts for a rectangle depend only on the
//! pre-erosion spans within [`reach`] cells of it: erosion runs over that window
//! alone, and a rebake of a rectangle grown by `reach` reproduces a full bake there.
//!
//! Deterministic: a Dijkstra with integer costs and a total order on its queue.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::super::{NavigationGraph, SpanRef};
use super::columns::Columns;
use super::region::CellRect;

const CARDINALS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const DIAGONALS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// The radius, in cells, the agent footprint erodes.
pub(super) fn radius_cells(agent_radius: f32, spacing: f32) -> i32 {
    ((agent_radius / spacing).ceil() as i32).max(0)
}

/// How far (cells) a change to the pre-erosion spans can move an erosion verdict.
pub(super) fn reach(agent_radius: f32, spacing: f32) -> i32 {
    radius_cells(agent_radius, spacing) + 1
}

impl NavigationGraph {
    /// The spans of `region` that survive erosion by `agent_radius`, read from this
    /// pre-erosion graph within `region` grown by [`reach`].
    pub(super) fn eroded_columns(&self, agent_radius: f32, region: CellRect) -> Columns {
        let radius = radius_cells(agent_radius, self.grid_spacing);
        if radius == 0 {
            return self.columns(region);
        }
        let threshold = 2 * radius as u32;
        let window = region
            .grown(reach(agent_radius, self.grid_spacing))
            .clamped(self)
            .unwrap_or(region);
        let dist = self.boundary_distances(window, threshold);
        let mut out = Columns::with_capacity(region.cells());
        for gz in region.z0..=region.z1 {
            for gx in region.x0..=region.x1 {
                let range = self.span_range(gx, gz);
                let kept = range.filter(|&i| dist[i] >= threshold);
                out.push_column(kept.map(|i| self.spans[i]));
            }
        }
        out
    }

    /// Chamfer distance from the walkable edge for every span of `window`, capped at
    /// `threshold` (`u32::MAX` past it, and outside the window).
    fn boundary_distances(&self, window: CellRect, threshold: u32) -> Vec<u32> {
        let mut dist = vec![u32::MAX; self.spans.len()];
        let mut queue = BinaryHeap::new();
        for s in self.span_refs_in(window) {
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
                if nd < threshold && nd < dist[n.index as usize] && window.contains(n.gx, n.gz) {
                    dist[n.index as usize] = nd;
                    queue.push(Reverse((nd, n.index, n.gx, n.gz)));
                }
            }
        }
        dist
    }

    /// Every span of `window`'s cells, row-major.
    fn span_refs_in(&self, window: CellRect) -> impl Iterator<Item = SpanRef> + '_ {
        (window.z0..=window.z1).flat_map(move |gz| {
            (window.x0..=window.x1).flat_map(move |gx| {
                self.span_range(gx, gz).map(move |i| SpanRef {
                    gx,
                    gz,
                    index: i as u32,
                })
            })
        })
    }
}
