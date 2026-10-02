//! src/navigation/bake/carve.rs — cut carving obstacles out of the spans (#456).
//!
//! Unity's `NavMeshObstacle` carving. A carving obstacle's volume is rasterised
//! like a convex collider, one solid interval per cell column it reaches. Every
//! pre-erosion span whose standing room (`y .. y + agent_height`) that interval
//! touches is removed. Erosion then pulls the walkable surface back by the agent
//! radius around the hole, exactly as around a wall, so a path keeps an agent's
//! footprint clear of the obstacle.
//!
//! A span on another floor is untouched: a crate on the ground floor carves
//! nothing out of the floor above it. Carving runs per column, before erosion, so
//! it rebakes with the rest of a dirty rectangle.

use super::super::obstacle::ObstacleVolume;
use super::super::{NavSpan, NavigationGraph};
use super::columns::Columns;
use super::raster::{self, Solid};
use super::region::CellRect;

/// How far below a floor an obstacle's top may sit and still carve it: rasterised
/// floors are exact, so this only absorbs float error.
const CARVE_TOLERANCE: f32 = 0.05;

/// Remove from `cols` (the columns of `region`) every span a carving `obstacle`
/// reaches. `g` supplies the grid and the agent height.
pub(super) fn carve(
    g: &NavigationGraph,
    cols: &mut Columns,
    region: CellRect,
    obstacles: &[&ObstacleVolume],
) {
    let mut solids = Vec::new();
    for o in obstacles {
        let tris = o.triangles().into_iter();
        raster::rasterize_mesh(g, region, tris, true, |_| false, &mut solids);
    }
    if solids.is_empty() {
        return;
    }
    solids.sort_by_key(|s| s.cell);
    let mut out = Columns::with_capacity(region.cells());
    let mut next = 0;
    let mut local = 0;
    for gz in region.z0..=region.z1 {
        for gx in region.x0..=region.x1 {
            let cell = g.index(gx, gz) as u32;
            let end = next + solids[next..].partition_point(|s| s.cell == cell);
            let cutters = &solids[next..end];
            let kept = cols.column(local).iter().copied();
            out.push_column(kept.filter(|s| !cutters.iter().any(|c| cuts(c, s, g.agent_height))));
            next = end;
            local += 1;
        }
    }
    *cols = out;
}

/// Whether an obstacle interval reaches into a span's standing room.
fn cuts(c: &Solid, s: &NavSpan, agent_height: f32) -> bool {
    c.min < s.y + agent_height && c.max >= s.y - CARVE_TOLERANCE
}
