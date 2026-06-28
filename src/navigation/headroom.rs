//! src/navigation/headroom.rs — the agent-height clearance pass of the bake (#278).
//!
//! The navmesh is a single-surface 2.5D height field with no notion of a ceiling, so
//! "is there enough headroom here?" is otherwise unrepresentable. This pass adds it
//! *by subtraction only*: for each walkable cell it measures the vertical clearance from
//! the supporting surface up to the lowest static collider overhead and, when that
//! clearance is below the agent height, marks the cell non-walkable — so an agent can't
//! path under a low overhang or through a crawlspace. It never adds a second walkable
//! layer; a true multi-level navmesh is explicitly out of scope.
//!
//! # Sampling model (single-surface, bounded)
//! "Geometry above a cell" is the set of static collider AABBs whose XZ footprint covers
//! the cell. Among those, a collider is an **overhead ceiling** when its bottom
//! (`aabb_min.y`) sits strictly above the **support** beneath it — the highest covering
//! collider top that is at or below that bottom (else flat ground `0.0`). Clearance is
//! `ceiling_bottom - support`, and a cell is carved when the *lowest* such clearance is
//! below the agent height.
//!
//! Measuring against the support (not the global max-fold floor in `heightfield`) is what
//! makes the pass meaningful in a single-surface world: a floating slab raises the
//! `heightfield` to its own top via `raise_surface`, so comparing a ceiling's bottom to
//! that max-fold would be vacuous (the floor would always be at least the ceiling's top).
//! The support is the surface an agent would actually stand on under the overhang. A cell
//! with no covering collider above its support has clearance ∞ and is never carved, so
//! simple scenes (floor + walls, no ceilings) bake byte-identically (back-compat).
//! Pure grid + AABB math — no RNG/clock — so the determinism guard for `navigation` holds.

use super::NavigationGraph;
use crate::scene::Scene;
use glam::Vec3;

/// A collider whose top sits within this of a support level counts as resting on it (not
/// floating above it), so a stacked step / the collider that defines the floor is never
/// mistaken for its own ceiling. Also the strict-above margin for "overhead".
const SUPPORT_EPSILON: f32 = 1e-3;

/// One static collider's world AABB, reduced to the fields the headroom pass needs.
struct Overhead {
    min: Vec3,
    max: Vec3,
}

impl NavigationGraph {
    /// Carve every walkable cell whose vertical clearance to the lowest overhead collider
    /// is below `scene.nav_settings.agent_height` (#278). See the module docs for the
    /// single-surface sampling model; a no-op for any cell with nothing overhead.
    pub(super) fn carve_low_headroom(&mut self, scene: &Scene) {
        let agent_height = scene.nav_settings.agent_height;
        if agent_height <= 0.0 {
            return; // height 0 (or negative): nothing can be "too low", an exact no-op.
        }
        let colliders = collect_overhead(scene);
        for gz in 0..self.height {
            for gx in 0..self.width {
                let idx = self.index(gx, gz);
                if self.walkability[idx] && self.cell_clearance(gx, gz, &colliders) < agent_height {
                    self.walkability[idx] = false;
                }
            }
        }
    }

    /// Clearance at a cell: the smallest `ceiling_bottom - support` over every collider
    /// overhead of this cell's column, or `∞` when none is. The support of a candidate
    /// ceiling is the highest *other* covering collider top at or below the ceiling's
    /// bottom (else ground `0.0`); the ceiling counts only when its bottom is strictly
    /// above that support, so a grounded/stacked collider is never its own ceiling.
    fn cell_clearance(&self, gx: i32, gz: i32, colliders: &[Overhead]) -> f32 {
        let center = self.grid_to_world(gx, gz);
        let mut clearance = f32::INFINITY;
        for c in colliders {
            if !covers(c, center) {
                continue;
            }
            let support = self.support_below(center, c.min.y, colliders);
            if c.min.y > support + SUPPORT_EPSILON {
                clearance = clearance.min(c.min.y - support);
            }
        }
        clearance
    }

    /// The surface an agent would stand on under a ceiling whose bottom is `ceiling_bottom`:
    /// the highest covering collider top that is at or (within epsilon) below it, else flat
    /// ground `0.0`. Excludes the ceiling itself (a top above `ceiling_bottom` is the
    /// ceiling or higher geometry, not support).
    fn support_below(&self, center: Vec3, ceiling_bottom: f32, colliders: &[Overhead]) -> f32 {
        let mut support = 0.0_f32;
        for c in colliders {
            if covers(c, center) && c.max.y <= ceiling_bottom + SUPPORT_EPSILON && c.max.y > support
            {
                support = c.max.y;
            }
        }
        support
    }
}

/// Gather every active static collider's AABB (the same gate `bake` uses for the floor).
fn collect_overhead(scene: &Scene) -> Vec<Overhead> {
    let mut out = Vec::new();
    for entity in scene.iter() {
        if !entity.active || !entity.is_static {
            continue;
        }
        if let Some(col) = &entity.collider {
            if col.active {
                out.push(Overhead {
                    min: col.aabb_min,
                    max: col.aabb_max,
                });
            }
        }
    }
    out
}

/// Whether `center`'s XZ lies within the collider's footprint (cell-center containment,
/// matching `raise_surface`).
fn covers(c: &Overhead, center: Vec3) -> bool {
    center.x >= c.min.x && center.x <= c.max.x && center.z >= c.min.z && center.z <= c.max.z
}
