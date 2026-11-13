//! A held flood across an incremental rebake (#2116), carried as the path queue
//! carries its search (`request::carry`, #2087).
//!
//! The environment query's time-sliced form holds one flood across ticks. A
//! moving carving obstacle rebakes every few ticks, and a flood restarted on each
//! rebake never finishes behind one longer than a few ticks' budget: the query
//! queue, strictly first come first served, would leave every query waiting. So
//! a flood that nothing the rebakes changed reaches is renumbered onto the new
//! spans with the same cell-by-cell remap the path queue's search uses
//! ([`NavigationGraph::spliced`]), and goes on where it was.
//!
//! "Reaches" means one of its spans (one it costed, its start, a goal) lies near
//! a changed cell, or a goal point snaps elsewhere now, or the island map keeps
//! other goals than it did. A carried flood's costs hold on the new navmesh: every
//! move between the spans it reached is the move it was, so it answers what a
//! flood begun on the new navmesh answers. Its heap keeps each entry's key, so
//! only ties may pop in another order, and a tie never changes a settled cost.
//!
//! The renumbering counts from the cell offsets of the bake the flood was on,
//! copied once per bake while it is held (`FloodSearch::cells`): a flood run to
//! its end in one call copies nothing.

use super::FloodSearch;
use crate::NavigationGraph;

impl NavigationGraph {
    /// Carry `s`, last stepped on an earlier bake, onto this one: `true` when it
    /// was (see the module doc); `false`, leaving it to start over, when anything
    /// since changed what it holds or the change is unknown (a full bake, an
    /// area-cost change, a log that no longer reaches back).
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
