//! A search in flight is carried across an incremental rebake unless something it reached (a span it costed, or the goal) lies near a changed cell; its spans are renumbered cell by cell. A search a change does reach, a full bake or an area-cost change restarts.

use super::super::bake::CellRect;
use super::super::offmesh::LinkParams;
use super::super::{NavMeshSettings, NavigationGraph, SpanRef};
use super::PathQueue;

impl NavigationGraph {
    pub(crate) fn cells_before_rebake(&mut self) -> Option<Vec<u32>> {
        let searching = self.requests.get_mut().active.is_some();
        let mapped = self.islands.current(self.bake_generation, self.spans.len());
        (searching || mapped).then(|| self.cell_start.clone())
    }

    pub(crate) fn carry_search(
        &self,
        old_start: Option<&[u32]>,
        changed: &[CellRect],
        settings: &NavMeshSettings,
    ) {
        let Some(old_start) = old_start else {
            return;
        };
        let remap = self.spliced(old_start, changed, settings);
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
    /// A span index counted from `old_start`, renumbered onto the current spans: `None` for a span in a cell near enough a change that one of its moves may have changed.
    pub(crate) fn spliced<'a>(
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
