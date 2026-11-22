//! `radius_cells = ceil(agent_radius / spacing)` rounds up, so a partly covered cell still erodes.
//! Distances come from the pre-erosion surface (one shrink, not iterated); `radius_cells == 0` is a no-op.
//! Verdicts for a rectangle depend only on pre-erosion spans within [`reach`] cells of it.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::super::{NavigationGraph, SpanRef};
use super::columns::Columns;
use super::region::CellRect;

struct WindowIndex {
    window: CellRect,
    rows: Vec<(usize, usize)>,
    len: usize,
}

impl WindowIndex {
    fn new(g: &NavigationGraph, window: CellRect) -> Self {
        let mut rows = Vec::with_capacity((window.z1 - window.z0 + 1) as usize);
        let mut len = 0;
        for gz in window.z0..=window.z1 {
            let first = g.span_range(window.x0, gz).start;
            let end = g.span_range(window.x1, gz).end;
            rows.push((first, len));
            len += end - first;
        }
        Self { window, rows, len }
    }

    fn of(&self, gz: i32, i: usize) -> usize {
        let (first, local) = self.rows[(gz - self.window.z0) as usize];
        local + (i - first)
    }
}

const CARDINALS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const DIAGONALS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

pub(super) fn radius_cells(agent_radius: f32, spacing: f32) -> i32 {
    ((agent_radius / spacing).ceil() as i32).max(0)
}

pub(super) fn reach(agent_radius: f32, spacing: f32) -> i32 {
    radius_cells(agent_radius, spacing) + 1
}

impl NavigationGraph {
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
        let local = WindowIndex::new(self, window);
        let dist = self.boundary_distances(&local, threshold);
        let mut out = Columns::with_capacity(region.cells());
        for gz in region.z0..=region.z1 {
            for gx in region.x0..=region.x1 {
                let range = self.span_range(gx, gz);
                let kept = range.filter(|&i| dist[local.of(gz, i)] >= threshold);
                out.push_column(kept.map(|i| self.spans[i]));
            }
        }
        out
    }

    fn boundary_distances(&self, local: &WindowIndex, threshold: u32) -> Vec<u32> {
        let window = local.window;
        let mut dist = vec![u32::MAX; local.len];
        let mut queue = BinaryHeap::new();
        for s in self.span_refs_in(window) {
            if CARDINALS
                .iter()
                .any(|&(dx, dz)| self.neighbour(s, dx, dz).is_none())
            {
                dist[local.of(s.gz, s.index as usize)] = 0;
                queue.push(Reverse((0, s.index, s.gx, s.gz)));
            }
        }
        while let Some(Reverse((d, index, gx, gz))) = queue.pop() {
            if d > dist[local.of(gz, index as usize)] {
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
                if !window.contains(n.gx, n.gz) {
                    continue;
                }
                let nd = d + cost;
                let slot = &mut dist[local.of(n.gz, n.index as usize)];
                if nd < threshold && nd < *slot {
                    *slot = nd;
                    queue.push(Reverse((nd, n.index, n.gx, n.gz)));
                }
            }
        }
        dist
    }

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
