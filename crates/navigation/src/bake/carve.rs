use super::super::obstacle::ObstacleVolume;
use super::super::{NavSpan, NavigationGraph};
use super::columns::Columns;
use super::raster::{self, Solid};
use super::region::CellRect;

/// How far below a floor an obstacle's top may sit and still carve it: rasterised
/// floors are exact, so this only absorbs float error.
const CARVE_TOLERANCE: f32 = 0.05;

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

fn cuts(c: &Solid, s: &NavSpan, agent_height: f32) -> bool {
    c.min < s.y + agent_height && c.max >= s.y - CARVE_TOLERANCE
}
