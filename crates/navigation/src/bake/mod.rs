//! No solid, no span: there is no implicit ground past the geometry.
//! Rasterising, merging, carving and modifiers are per column and erosion reads a fixed
//! neighbourhood, so a change re-runs those over the dirty cells and erosion over those grown by
//! its reach, giving a full bake's result.
//! Deterministic: pure geometry, a total order on every sort, no RNG or clock.

mod carve;
mod columns;
mod erosion;
mod grow;
mod heightfield;
mod inputs;
mod modifiers;
mod plane;
mod raster;
mod region;
mod saved;
mod state;
mod still;
mod sync;

#[cfg(test)]
mod tests;

pub(crate) use columns::Columns;
pub use region::CellRect;
pub use saved::{baked_fingerprint, first_baked_span};
pub use state::BakeState;
pub use still::Start;
pub(super) use still::StaticWatch;
pub use sync::Rebake;

use super::obstacle::ObstacleVolume;
use super::offmesh::{authored_keys, AuthoredKey, LinkParams};
use super::NavigationGraph;
use crate::grid::TileKey;
use inputs::{collider_keys, obstacle_keys, sources, BakeInputs, Source};
use modifiers::{modifier_keys, ModifierVolume};
use rusty_physics::collider_world_triangles;
use rusty_scene::Scene;
use std::collections::BTreeMap;

impl NavigationGraph {
    /// Every tile baked on its own: rasterised from the colliders over it, then eroded reading
    /// its neighbours' pre-erosion spans, so no border shows.
    pub fn bake(&mut self, scene: &Scene) {
        let log = self.bake_state.take().map(|s| s.log).unwrap_or_default();
        self.reshape_for(scene);
        self.clear_spans();
        let settings = &scene.nav_settings;
        self.adopt_settings(settings);
        self.bake_generation = self.bake_generation.wrapping_add(1);

        let colliders = collider_keys(scene);
        let obstacles = obstacle_keys(scene);
        let modifiers = modifier_keys(scene);
        let mut raw = self.empty_like();
        let volumes: Vec<&ObstacleVolume> = obstacles.iter().map(|(_, v)| v).collect();
        let boxes: Vec<&ModifierVolume> = modifiers.iter().map(|(_, v)| v).collect();
        let rects: Vec<Option<CellRect>> = colliders
            .iter()
            .map(|&(id, _)| collider_world_triangles(scene, id))
            .map(|mesh| mesh.and_then(|m| raster::footprint(&raw, &m.vertices)))
            .collect();
        let mut by_tile: BTreeMap<TileKey, Vec<u32>> = BTreeMap::new();
        for (&(id, _), rect) in colliders.iter().zip(&rects) {
            let Some(r) = rect.and_then(|r| r.clamped(&raw)) else {
                continue;
            };
            for key in raw.tiles.keys_over(r) {
                by_tile.entry(key).or_default().push(id);
            }
        }
        for (&key, ids) in &by_tile {
            if let Some(tile) = raw.tiles.rect(key) {
                raw.rebuild_raw(scene, ids, (&volumes, &boxes), tile);
            }
        }
        for key in raw.tiles.keys() {
            if let Some(tile) = raw.tiles.rect(key) {
                let eroded = raw.eroded_columns(settings.agent_radius, tile);
                self.splice(tile, &eroded);
            }
        }
        self.offmesh.auto = match LinkParams::new(settings, self.grid_spacing) {
            Some(p) => self.generate_links(&raw, &p, CellRect::all(self)),
            None => Vec::new(),
        };
        let links = authored_keys(scene);
        self.offmesh.authored = self.resolve_authored(&links);
        self.index_links();

        let colliders = colliders.into_iter().zip(rects);
        let colliders = colliders.map(|((id, key), rect)| Source { id, key, rect });
        let keys = (obstacles, modifiers, links);
        let inputs = record_inputs(&raw, settings, colliders.collect(), keys);
        let mut state = BakeState { raw, inputs, log };
        state.log.record(self.bake_generation, None);
        self.bake_state = Some(Box::new(state));
    }

    pub(crate) fn clear_spans(&mut self) {
        self.tiles = self.tiles.emptied();
        self.spans.clear();
    }

    fn rebuild_raw(
        &mut self,
        scene: &Scene,
        ids: &[u32],
        (obstacles, modifiers): (&[&ObstacleVolume], &[&ModifierVolume]),
        region: CellRect,
    ) -> Vec<Option<CellRect>> {
        let (solids, rects) = raster::rasterize(self, scene, ids, region);
        let mut cols = self.build_columns(solids, region);
        carve::carve(self, &mut cols, region, obstacles);
        modifiers::apply(self, &mut cols, region, modifiers);
        self.splice(region, &cols);
        rects
    }

    fn adopt_settings(&mut self, settings: &super::NavMeshSettings) {
        self.max_step = settings.max_step;
        self.max_slope = settings.max_slope;
        self.agent_height = settings.agent_height.max(0.0);
        self.area_costs = super::cost_table(&settings.areas);
    }

    fn empty_like(&self) -> Self {
        let mut g = Self::empty(
            self.min_x,
            self.max_x,
            self.min_z,
            self.max_z,
            self.grid_spacing,
            self.tiles.side,
        );
        g.max_step = self.max_step;
        g.max_slope = self.max_slope;
        g.agent_height = self.agent_height;
        g
    }
}

fn record_inputs(
    g: &NavigationGraph,
    settings: &super::NavMeshSettings,
    colliders: Vec<Source<inputs::ColliderKey>>,
    (obstacles, modifiers, links): InputKeys,
) -> BakeInputs {
    BakeInputs {
        settings: settings.clone(),
        colliders,
        obstacles: sources(obstacles, |v| raster::footprint(g, &v.triangles().concat())),
        modifiers: sources(modifiers, |v| v.footprint(g)),
        links: sources(links, |k| link_rect(g, k)),
    }
}

type InputKeys = (
    Vec<(u32, ObstacleVolume)>,
    Vec<(u32, ModifierVolume)>,
    Vec<(u32, AuthoredKey)>,
);

fn link_rect(g: &NavigationGraph, k: &AuthoredKey) -> Option<CellRect> {
    CellRect::covering(g, k.start.min(k.end), k.start.max(k.end))
}
