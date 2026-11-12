//! The island map across a change that leaves most of it standing (#2111).
//!
//! Building the map walks every span's eight steps: about 17 ms on a Dust2-sized
//! grid. A moving carving obstacle rebakes a few cells every few ticks, and an
//! area-cost change (a molotov zone) rebakes none, so neither rebuilds it:
//!
//! - An area-cost change moves no span and no link: the map holds as it is.
//! - An incremental rebake changes the cells it names. Every cell farther than one
//!   from them (the *kept* cells) holds the same spans, in order, at a new offset
//!   (`request::carry` renumbers a search the same way), and every step between two
//!   kept cells is the step it was. So a kept span keeps its old area, and the spans
//!   of the cells within one of the change (the *fresh* ones) are joined anew by
//!   every step that touches them, which merges each with the old areas around it.
//!   That joins at least everything a fresh build joins: a split the change made is
//!   never seen (two old areas stay one), a join always is. Merging too much only
//!   gives a search toward an unreachable goal the full cap, so it is safe; merging
//!   too little would cap a reachable goal, and cannot happen. The link exits are
//!   rebuilt from the links, one pass over them.
//!
//! Anything else (a full bake, a reshape, a restored bake) rebuilds on the next
//! search.

use super::super::super::bake::CellRect;
use super::super::super::{NavigationGraph, SpanRef};
use super::areas::Roots;
use super::{IslandMap, Islands};

impl Islands {
    /// Whether the cached map answers for bake `generation` of `spans` spans.
    pub(in crate::navigation) fn current(&self, generation: u64, spans: usize) -> bool {
        let map = self.map.borrow();
        map.as_ref()
            .is_some_and(|m| m.generation == generation && m.area.len() == spans)
    }

    /// The bake moved on to `generation` without moving a span or a link (an area
    /// cost changed): a map of the bake before still holds.
    pub(in crate::navigation) fn keep(&mut self, generation: u64) {
        if let Some(m) = self.map.get_mut() {
            if m.generation.wrapping_add(1) == generation {
                m.generation = generation;
            }
        }
    }
}

impl NavigationGraph {
    /// After an incremental rebake of `changed`: carry the island map onto the new
    /// spans (see the module doc), or drop it to be rebuilt. `old_start` is the cell
    /// offsets before the rebake (`cells_before_rebake`), `None` when the map was
    /// not current then.
    pub(in crate::navigation) fn carry_islands(
        &mut self,
        old_start: Option<&[u32]>,
        changed: &[CellRect],
    ) {
        let old = self.islands.map.get_mut().take();
        let map = old
            .zip(old_start)
            .and_then(|(old, start)| self.carried(&old, start, changed));
        *self.islands.map.get_mut() = map;
    }

    /// `old` carried onto the current spans; `None` when it does not fit them.
    fn carried(
        &self,
        old: &IslandMap,
        old_start: &[u32],
        changed: &[CellRect],
    ) -> Option<IslandMap> {
        let fits = old.generation.wrapping_add(1) == self.bake_generation
            && old_start.len() == self.cell_start.len()
            && old_start
                .last()
                .is_some_and(|&n| n as usize == old.area.len());
        let fresh = fits.then(|| self.fresh_cells(changed))?;
        let old_count = old.exits.len() as u32;
        let node = |s: SpanRef| {
            let c = self.index(s.gx, s.gz);
            match fresh[c] {
                true => old_count + s.index,
                false => old.area[(old_start[c] + s.index - self.cell_start[c]) as usize],
            }
        };
        let mut roots = Roots::new(old_count as usize + self.spans.len());
        let touched: Vec<CellRect> = changed
            .iter()
            .filter_map(|r| r.grown(2).clamped(self))
            .collect();
        for r in &touched {
            for here in self.spans_in(*r) {
                self.join_cardinals(here, &mut roots, node);
            }
        }
        for r in &touched {
            for here in self.spans_in(*r) {
                self.join_diagonals(here, &mut roots, node);
            }
        }
        let (area, count) = self.relabel(old, old_start, &fresh, &mut roots)?;
        let exits = self.link_exits(&area, count);
        Some(IslandMap {
            generation: self.bake_generation,
            area,
            exits,
        })
    }

    /// Each cell's flag: within one cell of a change.
    fn fresh_cells(&self, changed: &[CellRect]) -> Vec<bool> {
        let mut fresh = vec![false; self.cell_start.len() - 1];
        for r in changed.iter().filter_map(|r| r.grown(1).clamped(self)) {
            for gz in r.z0..=r.z1 {
                let row = self.index(r.x0, gz)..=self.index(r.x1, gz);
                fresh[row].iter_mut().for_each(|f| *f = true);
            }
        }
        fresh
    }

    /// Every span's area from the joined `roots`, numbered in scan order, and how
    /// many there are; `None` when a kept cell's spans moved (they never should).
    /// A run of kept cells is one stretch of old spans at a new offset, each reading
    /// its old area's root, found once per area.
    fn relabel(
        &self,
        old: &IslandMap,
        old_start: &[u32],
        fresh: &[bool],
        roots: &mut Roots,
    ) -> Option<(Vec<u32>, usize)> {
        let old_count = old.exits.len() as u32;
        // Each root's label plus one, 0 for none yet: zeroed, like `roots`.
        let mut label = vec![0u32; old_count as usize + self.spans.len()];
        let mut count = 0u32;
        let mut name = |roots: &mut Roots, node: u32| {
            let r = roots.find(node) as usize;
            if label[r] == 0 {
                count += 1;
                label[r] = count;
            }
            label[r] - 1
        };
        let mut old_label = vec![u32::MAX; old_count as usize];
        let mut area = vec![0; self.spans.len()];
        let mut c0 = 0;
        for run in fresh.chunk_by(|a, b| a == b) {
            let c1 = c0 + run.len();
            let new = self.cell_start[c0] as usize..self.cell_start[c1] as usize;
            if run[0] {
                for i in new {
                    area[i] = name(roots, old_count + i as u32);
                }
            } else {
                let (o, n) = (old_start[c0], self.cell_start[c0]);
                let mut cells = old_start[c0..=c1].iter().zip(&self.cell_start[c0..=c1]);
                if !cells.all(|(&a, &b)| a - o == b - n) {
                    return None;
                }
                let from = &old.area[o as usize..][..new.len()];
                for (to, &a) in area[new].iter_mut().zip(from) {
                    let a = a as usize;
                    if old_label[a] == u32::MAX {
                        old_label[a] = name(roots, a as u32);
                    }
                    *to = old_label[a];
                }
            }
            c0 = c1;
        }
        Some((area, count as usize))
    }

    /// The spans of the cells of `r`, in storage order.
    fn spans_in(&self, r: CellRect) -> impl Iterator<Item = SpanRef> + '_ {
        (r.z0..=r.z1).flat_map(move |gz| {
            (r.x0..=r.x1).flat_map(move |gx| {
                self.span_range(gx, gz).map(move |i| SpanRef {
                    gx,
                    gz,
                    index: i as u32,
                })
            })
        })
    }
}
