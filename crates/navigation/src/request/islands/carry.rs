//! Fresh cells (within one of a change) are joined anew by every step touching them; kept cells
//! keep their old area. A split the change made is never seen (two old areas stay one), a join
//! always is: merging too much only gives an unreachable-goal search the full cap, merging too
//! little cannot happen.

use super::super::super::bake::CellRect;
use super::super::super::{NavigationGraph, SpanRef};
use super::super::Layout;
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
    pub(crate) fn carry_islands(&mut self, old_start: Option<&Layout>, changed: &[CellRect]) {
        let old = self.islands.map.get_mut().take();
        let map = old
            .zip(old_start)
            .and_then(|(old, start)| self.carried(&old, start, changed));
        *self.islands.map.get_mut() = map;
        self.islands.quiet = 0;
    }

    fn carried(&self, old: &IslandMap, before: &Layout, changed: &[CellRect]) -> Option<IslandMap> {
        let fits = old.generation.wrapping_add(1) == self.bake_generation
            && before.tiles.same_frame(&self.tiles)
            && before.ids == old.area.len();
        let fresh: Vec<CellRect> = fits.then(|| {
            changed
                .iter()
                .filter_map(|r| r.grown(1).clamped(self))
                .collect()
        })?;
        let is_fresh = |gx: i32, gz: i32| fresh.iter().any(|r| r.contains(gx, gz));
        if !self.kept_cells_fit(before, is_fresh) {
            return None; // checked before any join reads `old.area` through `node`
        }
        let old_count = old.exits.len() as u32;
        let node = |s: SpanRef| match is_fresh(s.gx, s.gz) {
            true => old_count + s.index,
            false => old.area[self.old_id(before, s)],
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
        let (area, count) = self.relabel(old, before, is_fresh, &mut roots);
        let exits = self.link_exits(&area, count);
        Some(IslandMap {
            generation: self.bake_generation,
            area,
            exits,
            carried: true,
        })
    }

    /// The id a kept span had before the rebake: kept cells hold the same spans.
    fn old_id(&self, before: &Layout, s: SpanRef) -> usize {
        before.range(s.gx, s.gz).start + (s.index as usize - self.span_range(s.gx, s.gz).start)
    }

    fn kept_cells_fit(&self, before: &Layout, fresh: impl Fn(i32, i32) -> bool) -> bool {
        (0..self.height).all(|gz| {
            (0..self.width).all(|gx| {
                fresh(gx, gz) || before.range(gx, gz).len() == self.span_range(gx, gz).len()
            })
        })
    }

    fn relabel(
        &self,
        old: &IslandMap,
        before: &Layout,
        fresh: impl Fn(i32, i32) -> bool,
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
        for s in self.span_refs() {
            area[s.index as usize] = if fresh(s.gx, s.gz) {
                name(roots, old_count + s.index)
            } else {
                let a = old.area[self.old_id(before, s)] as usize;
                if old_label[a] == u32::MAX {
                    old_label[a] = name(roots, a as u32);
                }
                old_label[a]
            };
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
