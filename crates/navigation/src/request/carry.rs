//! A search in flight is carried across an incremental rebake unless something it reached (a span
//! it costed, or the goal) lies near a changed cell; its spans are renumbered cell by cell. A
//! search a change does reach, a full bake, a bounds change or an area-cost change restarts.

use super::super::bake::CellRect;
use super::super::grid::TileGrid;
use super::super::offmesh::LinkParams;
use super::super::{NavMeshSettings, NavigationGraph, SpanRef};
use super::PathQueue;

/// Where every span was before a rebake: its tiles' columns, and how many ids there were.
#[derive(Clone)]
pub(crate) struct Layout {
    pub tiles: TileGrid,
    pub ids: usize,
}

impl Layout {
    pub fn range(&self, gx: i32, gz: i32) -> std::ops::Range<usize> {
        let inside = gx >= 0 && gz >= 0 && gx < self.tiles.width && gz < self.tiles.height;
        match inside {
            true => self.tiles.range(gx, gz),
            false => 0..0,
        }
    }
}

impl NavigationGraph {
    pub(crate) fn layout(&self) -> Layout {
        Layout {
            tiles: self.tiles.clone(),
            ids: self.spans.len(),
        }
    }

    pub(crate) fn cells_before_rebake(&mut self) -> Option<Layout> {
        let searching = self.requests.get_mut().active.is_some();
        let mapped = self.islands.current(self.bake_generation, self.spans.len());
        (searching || mapped).then(|| self.layout())
    }

    pub(crate) fn carry_search(
        &self,
        old_start: Option<&Layout>,
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
    /// A span id of the `old` layout, renumbered onto the current spans: `None` for a span in a
    /// cell near enough a change that one of its moves may have changed.
    pub(crate) fn spliced<'a>(
        &'a self,
        old: &'a Layout,
        changed: &[CellRect],
        settings: &NavMeshSettings,
    ) -> impl Fn(u32) -> Option<SpanRef> + 'a {
        let links = LinkParams::new(settings, self.grid_spacing);
        let origin = self.tiles.origin;
        let near: Vec<CellRect> = changed
            .iter()
            .map(|&r| links.map_or(r.grown(1), |p| p.region(r, origin)))
            .collect();
        let framed = old.tiles.same_frame(&self.tiles);
        let locate = old.tiles.locator();
        move |index: u32| -> Option<SpanRef> {
            let (gx, gz, off) = framed.then(|| locate(index)).flatten()?;
            if near.iter().any(|r| r.contains(gx, gz)) {
                return None;
            }
            let now = self.span_range(gx, gz);
            let index = now.start as u32 + off;
            (index < now.end as u32).then_some(SpanRef { gx, gz, index })
        }
    }
}
