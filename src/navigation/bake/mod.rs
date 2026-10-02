//! src/navigation/bake/ — building the layered navmesh from the scene (#454).
//!
//! The bake is Recast's front half on the existing grid:
//!
//! 1. `raster` — static collider triangles (the shapes physics collides with) are
//!    clipped into each cell column as solid spans, tagged walkable when an
//!    up-facing, not-too-steep triangle forms their top.
//! 2. `heightfield` — each column's solids merge; every walkable top with at least
//!    `agent_height` of open space above it becomes a span. No solid, no span: there
//!    is no implicit ground past the geometry (#666).
//! 3. `carve` — carving `NavMeshObstacle`s cut their footprint out (#456), then
//!    `modifiers` — `NavMeshModifierVolume`s assign their area to the spans inside
//!    them, and `NotWalkable` ones remove them (#460).
//! 4. `erosion` — spans within `agent_radius` of the surface's edge are dropped.
//! 5. off-mesh links (#462) — drops and jumps are generated along the ledges of the
//!    result, and authored links snap onto it (`navigation::offmesh`).
//!
//! Steps 1–3 are per column; erosion reads a fixed neighbourhood. That makes the
//! bake **incremental** (#456, `sync`): the graph keeps its pre-erosion spans and a
//! record of its inputs (`state`), and a change re-runs 1–3 over the dirty cells
//! and erosion over those grown by its reach — the same result a full bake gives.
//!
//! Deterministic: pure geometry, a total order on every sort, no RNG or clock.

mod carve;
mod columns;
mod erosion;
mod heightfield;
mod inputs;
mod modifiers;
mod raster;
mod region;
mod state;
mod sync;

#[cfg(test)]
mod tests;

pub use region::CellRect;
pub use state::BakeState;
pub use sync::Rebake;

use super::obstacle::ObstacleVolume;
use super::offmesh::{authored_keys, AuthoredKey, LinkParams};
use super::NavigationGraph;
use crate::scene::Scene;
use inputs::{collider_keys, obstacle_keys, sources, BakeInputs, Source};
use modifiers::{modifier_keys, ModifierVolume};

impl NavigationGraph {
    /// Re-bake the walkable spans from the scene's static colliders, its carving
    /// obstacles and its `nav_settings`, re-shaping the grid to the scene's bounds
    /// first (#452). Always the whole grid: [`Self::sync`] rebakes only what changed.
    pub fn bake(&mut self, scene: &Scene) {
        let log = self.bake_state.take().map(|s| s.log).unwrap_or_default();
        self.reshape_for(scene);
        let settings = &scene.nav_settings;
        self.max_step = settings.max_step;
        self.max_slope = settings.max_slope;
        self.agent_height = settings.agent_height.max(0.0);
        self.area_costs = super::cost_table(&settings.areas);
        // A new bake may change any span, so agents planned against an older one
        // re-plan (#126).
        self.bake_generation = self.bake_generation.wrapping_add(1);

        let all = CellRect::all(self);
        let colliders = collider_keys(scene);
        let obstacles = obstacle_keys(scene);
        let mut raw = self.empty_like();
        let ids: Vec<u32> = colliders.iter().map(|&(id, _)| id).collect();
        let volumes: Vec<&ObstacleVolume> = obstacles.iter().map(|(_, v)| v).collect();
        let modifiers = modifier_keys(scene);
        let boxes: Vec<&ModifierVolume> = modifiers.iter().map(|(_, v)| v).collect();
        let rects = raw.rebuild_raw(scene, &ids, (&volumes, &boxes), all);
        let eroded = raw.eroded_columns(settings.agent_radius, all);
        self.cell_start = eroded.cell_start;
        self.spans = eroded.spans;
        self.offmesh.auto = match LinkParams::new(settings, self.grid_spacing) {
            Some(p) => self.generate_links(&raw, &p, all),
            None => Vec::new(),
        };
        let links = authored_keys(scene);
        self.offmesh.authored = self.resolve_authored(&links);
        self.index_links();

        let colliders = colliders.into_iter().zip(rects);
        let inputs = BakeInputs {
            settings: settings.clone(),
            colliders: colliders
                .map(|((id, key), rect)| Source { id, key, rect })
                .collect(),
            obstacles: sources(obstacles, |v| {
                raster::footprint(&raw, &v.triangles().concat())
            }),
            modifiers: sources(modifiers, |v| v.footprint(&raw)),
            links: sources(links, |k| link_rect(self, k)),
        };
        let mut state = BakeState { raw, inputs, log };
        state.log.record(self.bake_generation, None);
        self.bake_state = Some(Box::new(state));
    }

    /// Rebuild the pre-erosion spans of `region` from the colliders `ids`, the
    /// carving obstacles and the modifier volumes that reach it, and splice them in.
    /// Returns each collider's footprint, in `ids` order.
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

    /// A graph of the same grid and traversal limits with no spans at all.
    fn empty_like(&self) -> Self {
        let mut g = Self::new(
            self.min_x,
            self.max_x,
            self.min_z,
            self.max_z,
            self.grid_spacing,
        );
        g.spans.clear();
        g.cell_start.fill(0);
        g.max_step = self.max_step;
        g.max_slope = self.max_slope;
        g.agent_height = self.agent_height;
        g
    }
}

/// The cells an authored link spans: the box its two ends bound.
fn link_rect(g: &NavigationGraph, k: &AuthoredKey) -> Option<CellRect> {
    CellRect::covering(g, k.start.min(k.end), k.start.max(k.end))
}
