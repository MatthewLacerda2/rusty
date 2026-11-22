//! Fresh cells (within one of a change) are joined anew by every step touching them; kept cells
//! keep their old area. A split the change made is never seen (two old areas stay one), a join
//! always is: merging too much only gives an unreachable-goal search the full cap, merging too
//! little cannot happen.

use super::super::super::bake::CellRect;
use super::super::super::{NavigationGraph, SpanRef};
use super::areas::Roots;
use super::{IslandMap, Islands};

/// The syncs without a change after which a carried map is rebuilt: one second at the 1/60 s tick.
pub(crate) const QUIET_SYNCS: u32 = 60;

impl Islands {
    pub(crate) fn still(&mut self) {
        let Some(m) = self.map.get_mut() else {
            return;
        };
        if !m.carried {
            return;
        }
        self.quiet += 1;
        if self.quiet >= QUIET_SYNCS {
            *self.map.get_mut() = None;
            self.quiet = 0;
        }
    }

    pub(crate) fn current(&self, generation: u64, spans: usize) -> bool {
        let map = self.map.borrow();
        map.as_ref()
            .is_some_and(|m| m.generation == generation && m.area.len() == spans)
    }

    pub(crate) fn keep(&mut self, generation: u64) {
        if let Some(m) = self.map.get_mut() {
            if m.generation.wrapping_add(1) == generation {
                m.generation = generation;
            }
        }
    }
}

impl NavigationGraph {
    pub(crate) fn carry_islands(&mut self, old_start: Option<&[u32]>, changed: &[CellRect]) {
        let old = self.islands.map.get_mut().take();
        let map = old
            .zip(old_start)
            .and_then(|(old, start)| self.carried(&old, start, changed));
        *self.islands.map.get_mut() = map;
        self.islands.quiet = 0;
    }

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
        if !self.kept_cells_fit(old_start, &fresh) {
            return None; // checked before any join reads `old.area` through `node`
        }
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
        let (area, count) = self.relabel(old, old_start, &fresh, &mut roots);
        let exits = self.link_exits(&area, count);
        Some(IslandMap {
            generation: self.bake_generation,
            area,
            exits,
            carried: true,
        })
    }

    fn kept_cells_fit(&self, old_start: &[u32], fresh: &[bool]) -> bool {
        let count = |s: &[u32], c: usize| s[c + 1] - s[c];
        (0..fresh.len()).all(|c| fresh[c] || count(old_start, c) == count(&self.cell_start, c))
    }

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

    fn relabel(
        &self,
        old: &IslandMap,
        old_start: &[u32],
        fresh: &[bool],
        roots: &mut Roots,
    ) -> (Vec<u32>, usize) {
        let old_count = old.exits.len() as u32;
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
                let o = old_start[c0];
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
        (area, count as usize)
    }

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
