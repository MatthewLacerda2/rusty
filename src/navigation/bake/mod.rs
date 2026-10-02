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
//! 3. `erosion` — spans within `agent_radius` of the surface's edge are dropped.
//!
//! Deterministic: pure geometry, a total order on every sort, no RNG or clock.

mod erosion;
mod heightfield;
mod raster;

#[cfg(test)]
mod tests;

use super::NavigationGraph;
use crate::scene::Scene;

impl NavigationGraph {
    /// Re-bake the walkable spans from the scene's static colliders and its
    /// `nav_settings`, re-shaping the grid to the scene's bounds first (#452).
    pub fn bake(&mut self, scene: &Scene) {
        self.reshape_for(scene);
        let settings = &scene.nav_settings;
        self.max_step = settings.max_step;
        self.max_slope = settings.max_slope;
        self.agent_height = settings.agent_height.max(0.0);
        // A new bake may change any span, so agents planned against an older one
        // re-plan (#126).
        self.bake_generation = self.bake_generation.wrapping_add(1);

        let solids = raster::rasterize(self, scene);
        self.build_spans(solids);
        self.erode_for_agent_radius(settings.agent_radius);
    }
}
