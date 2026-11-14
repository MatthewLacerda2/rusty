//! The NavMesh gizmo's shell half (#1910): `render` may not import `navigation`, so
//! the shell, which holds the graph, turns it into world-space triangles and hands
//! them to the renderer (`Renderer::set_navmesh`). Nothing is built while the gizmo
//! is off, or while the Game tab is the one shown.
//!
//! The graph is a layered grid, not a triangle mesh, so the drawing is one quad per
//! walkable span: the span's cell, its four corners on the span's floor plane
//! (`NavigationGraph::surface_y`, so a ramp tilts), lifted [`LIFT`] off the floor
//! against z-fighting. A span of the not-walkable area is skipped; every walkable
//! area draws the same colour.
//!
//! **Patched, not rebuilt.** Edit mode syncs the graph every frame and Play every
//! tick, so a dragged crate rebakes a few cells each frame. The grid is cut into
//! [`TILE`]-cell square tiles, one GPU buffer each; a new generation re-uploads
//! only the tiles its changed rectangles (`NavigationGraph::changes_since`) reach.
//! Everything is rebuilt only when the change log can't answer (a full bake, an
//! area-cost edit, a log that no longer reaches back) or the graph is another one
//! (its address or grid shape differs: `NavMeshSource`).

use glam::Vec3;

use rusty_core::application::{Gizmo, SceneGizmos};
use rusty_navigation::{NavigationGraph, SpanRef, NOT_WALKABLE_AREA};
use rusty_render::{NavMeshPatch, NavMeshRevision, NavMeshSource, Renderer};

/// How far above its floor a span's quad is drawn, in metres.
pub(crate) const LIFT: f32 = 0.03;

/// A tile's side, in cells: a moved crate dirties a few tiles, and a Dust2-sized
/// grid (801 × 801 cells) is 26 × 26 of them, one draw each.
pub(crate) const TILE: i32 = 32;

/// Hand `nav` to `renderer` while `gizmos` draws the navmesh, patching only what
/// changed since the last hand-off; returns the bytes uploaded.
pub(super) fn hand(renderer: &mut Renderer, gizmos: &SceneGizmos, nav: &NavigationGraph) -> u64 {
    if !gizmos.on(Gizmo::NavMesh) {
        return 0;
    }
    let Some(plan) = plan(renderer.navmesh_revision(), nav) else {
        return 0;
    };
    let tiles = plan.tiles.into_iter().map(|t| (t, tile_triangles(nav, t)));
    renderer.set_navmesh(NavMeshPatch {
        revision: plan.revision,
        reset: plan.reset,
        tiles,
    })
}

/// Which tiles to rebuild, and whether to start over (`reset`: the tile count).
pub(crate) struct Plan {
    pub revision: NavMeshRevision,
    pub reset: Option<usize>,
    pub tiles: Vec<usize>,
}

/// What brings a renderer showing `last` up to `nav`; `None` when it already is.
pub(crate) fn plan(last: Option<NavMeshRevision>, nav: &NavigationGraph) -> Option<Plan> {
    let revision = NavMeshRevision {
        source: source(nav),
        generation: nav.bake_generation,
    };
    let dirty = match last {
        Some(last) if last == revision => return None,
        Some(last) if last.source == revision.source => dirty_tiles(nav, last.generation),
        _ => None,
    };
    let count = (tiles_across(nav.width) * tiles_across(nav.height)) as usize;
    Some(match dirty {
        Some(tiles) => Plan {
            revision,
            reset: None,
            tiles,
        },
        None => Plan {
            revision,
            reset: Some(count),
            tiles: (0..count).collect(),
        },
    })
}

/// The graph's exact name: its address and its grid's shape.
fn source(nav: &NavigationGraph) -> NavMeshSource {
    NavMeshSource {
        instance: std::ptr::from_ref(nav) as usize,
        width: nav.width,
        height: nav.height,
        grid: [nav.min_x, nav.min_z, nav.grid_spacing].map(f32::to_bits),
    }
}

fn tiles_across(cells: i32) -> i32 {
    (cells.max(0) + TILE - 1) / TILE
}

/// The tiles every rebake after `generation` touched, sorted; `None` when the
/// change log can't say (then anything may have changed).
fn dirty_tiles(nav: &NavigationGraph, generation: u64) -> Option<Vec<usize>> {
    let across = tiles_across(nav.width);
    let clamp = |v: i32, cells: i32| v.min(cells - 1).max(0) / TILE;
    let mut tiles = Vec::new();
    for r in nav.changes_since(generation)? {
        for tz in clamp(r.z0, nav.height)..=clamp(r.z1, nav.height) {
            for tx in clamp(r.x0, nav.width)..=clamp(r.x1, nav.width) {
                tiles.push((tz * across + tx) as usize);
            }
        }
    }
    tiles.sort_unstable();
    tiles.dedup();
    Some(tiles)
}

/// Two triangles per walkable span of `tile`, three world positions each.
pub(crate) fn tile_triangles(nav: &NavigationGraph, tile: usize) -> Vec<Vec3> {
    let across = tiles_across(nav.width).max(1) as usize;
    let (x0, z0) = ((tile % across) as i32 * TILE, (tile / across) as i32 * TILE);
    let mut out = Vec::new();
    for gz in z0..(z0 + TILE).min(nav.height) {
        for gx in x0..(x0 + TILE).min(nav.width) {
            for index in nav.span_range(gx, gz) {
                if nav.spans[index].area != NOT_WALKABLE_AREA {
                    let s = SpanRef {
                        gx,
                        gz,
                        index: index as u32,
                    };
                    out.extend(quad(nav, s));
                }
            }
        }
    }
    out
}

/// The span's cell, its corners on its floor plane, as two triangles.
fn quad(nav: &NavigationGraph, s: SpanRef) -> [Vec3; 6] {
    let half = 0.5 * nav.grid_spacing;
    let c = nav.cell_center(s.gx, s.gz);
    let corner = |dx: f32, dz: f32| {
        let (x, z) = (c.x + dx * half, c.z + dz * half);
        Vec3::new(x, nav.surface_y(s, x, z) + LIFT, z)
    };
    let [a, b, cc, d] = [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ];
    [a, b, cc, a, cc, d]
}

#[cfg(test)]
#[path = "navmesh_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "navmesh_patch_tests.rs"]
mod patch_tests;
