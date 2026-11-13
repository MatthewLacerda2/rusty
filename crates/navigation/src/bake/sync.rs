//! crates/navigation/src/bake/sync.rs — rebake only what changed (#456).
//!
//! [`NavigationGraph::sync`] runs every play tick (through `sync_changes`, `still`)
//! in place of the old full rebake every 60 frames. It diffs the scene against the
//! inputs the last bake read (`inputs`): nothing changed costs one pass over the
//! obstacles, the modifier volumes (#460) and the authored links, plus one over the
//! static colliders unless the change log says none of them changed (#1707). A changed one dirties the cells
//! its old and new footprints cover; overlapping dirty rectangles merge. Each rectangle's
//! pre-erosion columns are rebuilt and spliced in, then erosion re-runs over the
//! rectangles grown by its reach, and those cells of the final graph are spliced.
//!
//! A settings change or a change of the grid's bounds or spacing is a full bake:
//! every cell moves or every span may differ. Connectivity is computed on demand
//! from the spans (`links.rs`), so there are no stored components or links to
//! recompute, locally or globally. Three things are carried instead: off-mesh links
//! (#462), the generated ones near the change regenerated and the authored ones
//! re-snapped; the agents' search in progress (`request::carry`, #2087); and the
//! island map (`request::islands::carry`, #2111), joined anew around the change
//! and built fresh after a quiet spell, which a sync without a change counts.

use super::super::NavigationGraph;
use super::erosion;
use super::inputs::{collider_keys, diff, obstacle_keys};
use super::modifiers::{modifier_keys, ModifierVolume};
use super::raster;
use super::region::{merge_overlapping, CellRect};
use super::{link_rect, BakeState, ObstacleVolume};
use crate::offmesh::{authored_keys, sort_generated, AuthoredKey, LinkEnd, LinkParams};
use rusty_physics::collider_world_triangles;
use rusty_scene::Scene;

/// What a [`NavigationGraph::sync`] did.
#[derive(Clone, Debug, PartialEq)]
pub enum Rebake {
    /// Nothing the bake reads changed.
    Unchanged,
    /// These cell rectangles were rebaked (erosion reach included).
    Incremental(Vec<CellRect>),
    /// The whole grid was rebaked.
    Full,
}

impl NavigationGraph {
    /// Bring the navmesh up to date with `scene`, rebaking only the cells whose
    /// inputs changed since the last bake. A graph that was never baked, or whose
    /// settings, bounds or spacing changed, is baked in full. The result is the
    /// same graph a full [`Self::bake`] of `scene` gives.
    ///
    /// Area costs (#460) are not a bake input: a changed cost is adopted here
    /// without rebaking anything, and only makes cached agent paths stale.
    pub fn sync(&mut self, scene: &Scene) -> Rebake {
        self.sync_inner(scene, false)
    }

    /// [`Self::sync`]; `statics_still` (the change log saw no static entity change
    /// since the last sync, `still`) skips the static collider diff and the
    /// reshape check, which read only that geometry and the settings.
    pub(super) fn sync_inner(&mut self, scene: &Scene, statics_still: bool) -> Rebake {
        let current = self.bake_state.as_ref().is_some_and(|st| {
            st.inputs.settings.bakes_like(&scene.nav_settings)
                && (statics_still || !self.needs_reshape(scene))
        });
        if current {
            self.set_area_costs(&scene.nav_settings.areas);
        }
        let Some(mut st) = self.bake_state.take().filter(|_| current) else {
            self.bake(scene);
            return Rebake::Full;
        };
        let dirty = st.update_inputs(scene, statics_still);
        let link_dirty = st.update_links(scene, self);
        if dirty.is_empty() && link_dirty.is_empty() {
            self.bake_state = Some(st);
            self.islands.still(); // a carried island map rebuilds after a quiet spell (#2111)
            return Rebake::Unchanged;
        }
        let before = self.cells_before_rebake();
        let mut changed = self.rebake_regions(scene, &mut st, dirty);
        self.offmesh.authored = self.resolve_authored(&st.inputs.link_keys());
        self.index_links();
        changed.extend(link_dirty);
        let changed = merge_overlapping(changed);
        self.bake_generation = self.bake_generation.wrapping_add(1);
        st.log.record(self.bake_generation, Some(changed.clone()));
        self.bake_state = Some(st);
        self.carry_search(before.as_deref(), &changed, &scene.nav_settings); // #2087
        self.carry_islands(before.as_deref(), &changed); // #2111
        Rebake::Incremental(changed)
    }

    /// Rebuild the spans of the `dirty` cells and erode around them, then regenerate
    /// the drop and jump links those cells can reach. Returns the cells whose final
    /// spans were rebuilt.
    fn rebake_regions(
        &mut self,
        scene: &Scene,
        st: &mut BakeState,
        dirty: Vec<CellRect>,
    ) -> Vec<CellRect> {
        if dirty.is_empty() {
            return dirty;
        }
        let regions = merge_overlapping(dirty);
        for &r in &regions {
            let ids = st.inputs.collider_ids_in(r);
            let volumes = st.inputs.obstacles_in(r);
            let boxes = st.inputs.modifiers_in(r);
            st.raw.rebuild_raw(scene, &ids, (&volumes, &boxes), r);
        }
        let radius = scene.nav_settings.agent_radius;
        let reach = erosion::reach(radius, self.grid_spacing);
        let grown = regions.iter().filter_map(|r| r.grown(reach).clamped(self));
        let changed = merge_overlapping(grown.collect());
        for &r in &changed {
            let cols = st.raw.eroded_columns(radius, r);
            self.splice(r, &cols);
        }
        self.regenerate_links(&st.raw, scene, &changed);
        changed
    }

    /// Replace the generated links leaving any cell a change to `changed` can reach
    /// (`LinkParams::region`) with freshly generated ones, in full-bake order.
    fn regenerate_links(&mut self, raw: &NavigationGraph, scene: &Scene, changed: &[CellRect]) {
        let Some(p) = LinkParams::new(&scene.nav_settings, self.grid_spacing) else {
            return;
        };
        let grown = changed.iter().filter_map(|&r| p.region(r).clamped(self));
        let regions = merge_overlapping(grown.collect());
        let inside = |e: LinkEnd| regions.iter().any(|r| r.contains(e.gx, e.gz));
        let mut links = std::mem::take(&mut self.offmesh.auto);
        links.retain(|l| !inside(l.from));
        for &r in &regions {
            links.extend(self.generate_links(raw, &p, r));
        }
        sort_generated(&mut links);
        self.offmesh.auto = links;
    }
}

impl BakeState {
    /// Diff the recorded inputs against `scene`, record the current ones, and
    /// return the cells whose inputs changed. With `statics_still` the static
    /// colliders are known unchanged and keep their records undiffed.
    fn update_inputs(&mut self, scene: &Scene, statics_still: bool) -> Vec<CellRect> {
        let mut dirty = Vec::new();
        let raw = &self.raw;
        if !statics_still {
            let collider_rect = |id: u32, _: &_| {
                let mesh = collider_world_triangles(scene, id)?;
                raster::footprint(raw, &mesh.vertices)
            };
            self.inputs.colliders = diff(
                &self.inputs.colliders,
                collider_keys(scene),
                collider_rect,
                &mut dirty,
            );
        }
        let obstacle_rect =
            |_: u32, v: &ObstacleVolume| raster::footprint(raw, &v.triangles().concat());
        let obstacles = diff(
            &self.inputs.obstacles,
            obstacle_keys(scene),
            obstacle_rect,
            &mut dirty,
        );
        let modifier_rect = |_: u32, v: &ModifierVolume| v.footprint(raw);
        let modifiers = diff(
            &self.inputs.modifiers,
            modifier_keys(scene),
            modifier_rect,
            &mut dirty,
        );
        self.inputs.obstacles = obstacles;
        self.inputs.modifiers = modifiers;
        dirty
    }

    /// Diff the recorded authored links against `scene`, record the current ones, and
    /// return the cells a changed link spanned (before or after).
    fn update_links(&mut self, scene: &Scene, g: &NavigationGraph) -> Vec<CellRect> {
        let mut dirty = Vec::new();
        let rect = |_: u32, k: &AuthoredKey| link_rect(g, k);
        self.inputs.links = diff(&self.inputs.links, authored_keys(scene), rect, &mut dirty);
        dirty
    }
}
