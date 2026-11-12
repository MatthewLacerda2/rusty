//! A search in flight across an incremental rebake (#2087).
//!
//! A moving carving obstacle rebakes every few ticks. Restarting the head search
//! each time would stall the queue for good behind a search longer than a few
//! ticks' budget, so a search is carried over instead whenever nothing it reached
//! (a span it costed, or the goal) lies near a changed cell: near enough that a
//! changed span or a regenerated off-mesh link could have changed one of its
//! moves. The splice renumbers the spans, so the carried search is renumbered too,
//! cell by cell (an untouched cell keeps its spans, in order, at a new offset). A
//! search a change does reach, a full bake, or an area-cost change restarts.

use super::super::bake::CellRect;
use super::super::offmesh::LinkParams;
use super::super::{NavMeshSettings, NavigationGraph, SpanRef};
use super::PathQueue;

impl PathQueue {
    /// Before a rebake rewrites the spans: the cell offsets the active search's
    /// span indices count from, or `None` with no search to carry.
    pub(in crate::navigation) fn cells_before_rebake(
        &self,
        cell_start: &[u32],
    ) -> Option<Vec<u32>> {
        self.active.as_ref().map(|_| cell_start.to_vec())
    }
}

impl NavigationGraph {
    /// After an incremental rebake of `changed`: carry the active search onto the
    /// new spans when no change reaches it (see the module doc), else drop it so it
    /// restarts on the new navmesh. `old_start` is
    /// [`PathQueue::cells_before_rebake`]'s.
    pub(in crate::navigation) fn carry_search(
        &self,
        old_start: Option<Vec<u32>>,
        changed: &[CellRect],
        settings: &NavMeshSettings,
    ) {
        let Some(old_start) = old_start else {
            return;
        };
        let remap = self.spliced(&old_start, changed, settings);
        let mut queue = self.requests.borrow_mut();
        let PathQueue { active, pool, .. } = &mut *queue;
        let Some(a) = active.as_mut() else {
            return;
        };
        if a.generation.wrapping_add(1) == self.bake_generation
            && a.search.remap(&remap)
            && pool.remap(self.spans.len(), &remap)
        {
            a.generation = self.bake_generation;
        } else {
            *active = None;
        }
    }
}

impl NavigationGraph {
    /// A span index counted from the cell offsets `old_start` (the bake before
    /// an incremental rebake of `changed`), renumbered onto the current spans:
    /// `None` for a span in a cell near enough a change that one of its moves may
    /// have changed (see the module doc). The held flood is carried with it too
    /// (`flood::carry`, #2116).
    pub(in crate::navigation) fn spliced<'a>(
        &'a self,
        old_start: &'a [u32],
        changed: &[CellRect],
        settings: &NavMeshSettings,
    ) -> impl Fn(u32) -> Option<SpanRef> + 'a {
        let links = LinkParams::new(settings, self.grid_spacing);
        let near: Vec<CellRect> = changed
            .iter()
            .map(|&r| links.map_or(r.grown(1), |p| p.region(r)))
            .collect();
        let width = self.width.max(1) as usize;
        move |index: u32| -> Option<SpanRef> {
            let cell = old_start.partition_point(|&s| s <= index).checked_sub(1)?;
            let (gx, gz) = ((cell % width) as i32, (cell / width) as i32);
            if near.iter().any(|r| r.contains(gx, gz)) || cell + 1 >= old_start.len() {
                return None;
            }
            let index = self.cell_start.get(cell)? + (index - old_start[cell]);
            Some(SpanRef { gx, gz, index })
        }
    }
}
