//! A moving carving obstacle rebakes every few ticks, so a flood restarted on each rebake would never
//! finish. A flood no change reaches is renumbered onto the new spans, as the path queue's search is,
//! and goes on where it was: every move between the spans it reached is unchanged, and its heap
//! keeps each entry's key, so only ties may pop in another order, which never changes a settled cost.
//! A change reaches it when it lies near a span it costed, its start or a goal, or when a goal snaps
//! elsewhere now or the island map differs.

use super::FloodSearch;
use crate::NavigationGraph;

impl NavigationGraph {
    pub(super) fn carry_flood(&self, s: &mut FloodSearch) -> bool {
        let (Some(from), Some(settings)) = (s.from, self.baked_settings()) else {
            return false;
        };
        if s.done || !s.cells_fresh || s.cells.len() != self.cell_start.len() {
            return false;
        }
        let Some(changed) = self.changes_since(s.key.0) else {
            return false;
        };
        let remap = self.spliced(&s.cells, &changed, settings);
        let moved: Option<Vec<_>> = s
            .open
            .iter()
            .map(|o| remap(o.node().index).map(|n| o.moved(n)))
            .collect();
        let (Some(moved), Some(from)) = (moved, remap(from.index)) else {
            return false;
        };
        if !s.goals.carry(self, from, &s.ask.goals, &remap)
            || !s.cost.remap(self.spans.len(), &remap)
        {
            return false;
        }
        (s.open, s.from) = (moved.into(), Some(from));
        s.key = (self.bake_generation, self.spans.len());
        s.cells_fresh = false;
        true
    }
}
