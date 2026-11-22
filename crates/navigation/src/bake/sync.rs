//! A settings change or a change of the grid's bounds or spacing is a full bake. Connectivity is
//! computed on demand from the spans, so only off-mesh links, the agents' search in progress and
//! the island map are carried.

use super::super::NavigationGraph;
use super::erosion;
use super::inputs::{diff, InputKeys};
use super::modifiers::ModifierVolume;
use super::raster;
use super::region::{merge_overlapping, CellRect};
use super::{link_rect, BakeState, ObstacleVolume};
use crate::offmesh::{sort_generated, AuthoredKey, LinkEnd, LinkParams};
use rusty_physics::collider_world_triangles;
use rusty_scene::Scene;

#[cfg(test)]
thread_local! {
    pub(crate) static STATIC_DIFFS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rebake {
    Unchanged,
    Incremental(Vec<CellRect>),
    Full,
}

impl NavigationGraph {
    /// Area costs are not a bake input: a changed cost is adopted here without rebaking, and only
    /// makes cached agent paths stale.
    pub fn sync(&mut self, scene: &Scene) -> Rebake {
        self.sync_inner(scene, false)
    }

    pub fn is_current_for(&self, scene: &Scene) -> bool {
        self.bakes_current(scene, false)
            && (self.bake_state.as_ref())
                .is_some_and(|st| st.inputs.are(InputKeys::of(scene, false)))
    }

    fn bakes_current(&self, scene: &Scene, statics_still: bool) -> bool {
        self.bake_state.as_ref().is_some_and(|st| {
            st.inputs.settings.bakes_like(&scene.nav_settings)
                && (statics_still || !self.needs_reshape(scene))
        })
    }

    pub(super) fn sync_inner(&mut self, scene: &Scene, statics_still: bool) -> Rebake {
        let current = self.bakes_current(scene, statics_still);
        if current {
            self.set_area_costs(&scene.nav_settings.areas);
        }
        let Some(mut st) = self.bake_state.take().filter(|_| current) else {
            self.bake(scene);
            return Rebake::Full;
        };
        let keys = InputKeys::of(scene, statics_still);
        let (dirty, link_dirty) = st.update_inputs(scene, keys, self);
        if dirty.is_empty() && link_dirty.is_empty() {
            self.bake_state = Some(st);
            self.islands.still();
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
        self.carry_search(before.as_deref(), &changed, &scene.nav_settings);
        self.carry_islands(before.as_deref(), &changed);
        Rebake::Incremental(changed)
    }

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
    fn update_inputs(
        &mut self,
        scene: &Scene,
        keys: InputKeys,
        g: &NavigationGraph,
    ) -> (Vec<CellRect>, Vec<CellRect>) {
        let InputKeys {
            colliders,
            obstacles,
            modifiers,
            links,
        } = keys;
        let mut dirty = Vec::new();
        let raw = &self.raw;
        if let Some(colliders) = colliders {
            #[cfg(test)]
            STATIC_DIFFS.with(|n| n.set(n.get() + 1));
            let collider_rect = |id: u32, _: &_| {
                let mesh = collider_world_triangles(scene, id)?;
                raster::footprint(raw, &mesh.vertices)
            };
            self.inputs.colliders =
                diff(&self.inputs.colliders, colliders, collider_rect, &mut dirty);
        }
        let obstacle_rect =
            |_: u32, v: &ObstacleVolume| raster::footprint(raw, &v.triangles().concat());
        let obstacles = diff(&self.inputs.obstacles, obstacles, obstacle_rect, &mut dirty);
        let modifier_rect = |_: u32, v: &ModifierVolume| v.footprint(raw);
        let modifiers = diff(&self.inputs.modifiers, modifiers, modifier_rect, &mut dirty);
        self.inputs.obstacles = obstacles;
        self.inputs.modifiers = modifiers;
        (dirty, self.update_links(links, g))
    }

    fn update_links(
        &mut self,
        links: Vec<(u32, AuthoredKey)>,
        g: &NavigationGraph,
    ) -> Vec<CellRect> {
        let mut dirty = Vec::new();
        let rect = |_: u32, k: &AuthoredKey| link_rect(g, k);
        self.inputs.links = diff(&self.inputs.links, links, rect, &mut dirty);
        dirty
    }
}
