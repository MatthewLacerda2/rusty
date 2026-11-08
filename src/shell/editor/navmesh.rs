//! The NavMesh gizmo's shell half (#1910): `render` may not import `navigation`, so
//! the shell, which holds the graph, turns it into world-space triangles and hands
//! them to the renderer with a revision (`Renderer::set_navmesh`), which uploads them
//! only when the revision moves. Nothing is built while the gizmo is off.
//!
//! The graph is a layered grid, not a triangle mesh, so the drawing is one quad per
//! walkable span: the span's cell, its four corners on the span's floor plane
//! (`NavigationGraph::surface_y`, so a ramp tilts), lifted [`LIFT`] off the floor
//! against z-fighting. A span of the not-walkable area is skipped; every walkable
//! area draws the same colour.

use glam::Vec3;

use crate::core::application::{Gizmo, SceneGizmos};
use crate::navigation::{NavigationGraph, NOT_WALKABLE_AREA};
use crate::render::Renderer;

/// How far above its floor a span's quad is drawn, in metres.
pub(crate) const LIFT: f32 = 0.03;

/// Hand `nav` to `renderer` while `gizmos` draws the navmesh; returns whether the
/// renderer rebuilt its buffer.
pub(super) fn hand(renderer: &mut Renderer, gizmos: &SceneGizmos, nav: &NavigationGraph) -> bool {
    gizmos.on(Gizmo::NavMesh) && renderer.set_navmesh(revision(nav), || triangles(nav))
}

/// What names the graph's walkable surface: its bake generation (every bake, rebake
/// and restore bumps it) folded with the grid's shape, so a graph replaced by one
/// whose generation restarted still reads as new. O(1): a frame never walks spans.
pub(crate) fn revision(nav: &NavigationGraph) -> u64 {
    let words = [
        nav.bake_generation,
        nav.width as u64,
        nav.height as u64,
        nav.spans.len() as u64,
        nav.min_x.to_bits() as u64,
        nav.min_z.to_bits() as u64,
        nav.grid_spacing.to_bits() as u64,
    ];
    // FNV-1a over the words: deterministic, no std hasher.
    words.iter().fold(0xcbf2_9ce4_8422_2325, |h, w| {
        (h ^ w).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Two triangles per walkable span, three world positions each.
pub(crate) fn triangles(nav: &NavigationGraph) -> Vec<Vec3> {
    let half = 0.5 * nav.grid_spacing;
    let mut out = Vec::with_capacity(nav.spans.len() * 6);
    for s in nav.span_refs() {
        if nav.spans[s.index as usize].area == NOT_WALKABLE_AREA {
            continue;
        }
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
        out.extend([a, b, cc, a, cc, d]);
    }
    out
}

#[cfg(test)]
#[path = "navmesh_tests.rs"]
mod tests;
