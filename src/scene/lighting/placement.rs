//! src/scene/lighting_placement.rs — Deterministic auto-placement of IBL probes.
//!
//! The "place well" half of the one-button bake (#246): given the STATIC scene (and,
//! when available, the baked nav surface), compute where light probes and reflection
//! probes should sit. This is a PURE, deterministic function of the scene/nav data —
//! a grid + an AABB subdivision, no RNG and no wall-clock — so the same level always
//! yields the same probe layout (the determinism the headless bake relies on).
//!
//! The bake itself (rendering each probe → SH / prefiltered cubemap) is the render/dev
//! half and lives in `dev::lighting_bake`; this module only decides POSITIONS + bounds.
//!
//! Two surfaces are placed:
//!   * **Light probes** — a regular grid through the navigable volume. Bounds come from
//!     the nav surface's walkable XZ extent (raised through a vertical headroom so
//!     actors at different elevations get coverage) when a baked nav graph is present;
//!     otherwise they fall back to the static-geometry AABB.
//!   * **Reflection probes** — one per coarse region, from an AABB subdivision of the
//!     static bounds. Each region yields a centroid + a parallax box covering it.
//!
//! This module is pure data + math — deterministic, sim-reachable, no RNG/clock.

use glam::Vec3;

use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// A light-probe grid request: the inclusive `[min, max]` corners and the cell spacing,
/// ready to hand to [`crate::scene::lighting::probe::ProbeVolume::fill_grid`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeGridPlan {
    pub min: Vec3,
    pub max: Vec3,
    pub spacing: f32,
}

/// A reflection-probe request: capture position (region centroid) plus the parallax
/// box corners covering the region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionPlan {
    pub position: Vec3,
    pub box_min: Vec3,
    pub box_max: Vec3,
}

/// Vertical headroom added above the placement bounds so probes also cover the air an
/// actor's head/torso occupies, not only the floor it stands on. World units.
const VERTICAL_HEADROOM: f32 = 3.0;

/// World-space AABB of the scene's STATIC, active geometry (its colliders). `None`
/// when the scene has no static collider to bound — the caller then has nothing to
/// place against. Deterministic: a pure fold over the entities in id order.
pub fn static_scene_aabb(scene: &Scene) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;
    for id in scene.entity_ids() {
        let Some(entity) = scene.get_entity(id) else {
            continue;
        };
        if !entity.is_static || !entity.active {
            continue;
        }
        let Some(collider) = &entity.collider else {
            continue;
        };
        if !collider.active {
            continue;
        }
        let world_mat = scene.world_matrix(id);
        let (cmin, cmax) = collider.calculate_world_aabb(world_mat);
        min = min.min(cmin);
        max = max.max(cmax);
        found = true;
    }
    found.then_some((min, max))
}

/// True when a nav graph has been baked into a usable walkable surface (a positive XZ
/// span). A default/unbaked graph has a degenerate extent and is skipped.
fn nav_is_baked(nav: &NavigationGraph) -> bool {
    nav.width > 1 && nav.height > 1 && nav.max_x > nav.min_x && nav.max_z > nav.min_z
}

/// AABB of the baked nav surface: the bounding box of the cells an agent can actually
/// stand on (the WALKABLE cells), with `y` spanning their height field (floor) up to
/// `floor + VERTICAL_HEADROOM` so probes cover the volume actors move through. Bounding
/// the *walkable* cells (not the full grid) means the agent-radius erosion (#277) tightens
/// this extent: as the walkable surface pulls back off walls and through thin passages,
/// the probe bounds follow where agents can actually stand. `None` when the graph is
/// unbaked or nothing is walkable.
fn nav_surface_aabb(nav: &NavigationGraph) -> Option<(Vec3, Vec3)> {
    if !nav_is_baked(nav) {
        return None;
    }
    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for gz in 0..nav.height {
        for gx in 0..nav.width {
            if !nav.is_walkable(gx, gz) {
                continue;
            }
            let w = nav.grid_to_world(gx, gz); // XZ cell center carried to its surface y
            min = min.min(w);
            max = max.max(w);
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return None; // no walkable cell to bound
    }
    max.y += VERTICAL_HEADROOM;
    Some((min, max))
}

/// Choose the light-probe grid bounds: the navigable volume intersected with the
/// scene's static AABB when a baked nav surface exists, else the static AABB alone,
/// else `None` (nothing to place). The intersection keeps the grid inside the level
/// rather than spilling past its walls when the nav extent is larger.
fn placement_bounds(scene: &Scene, nav: Option<&NavigationGraph>) -> Option<(Vec3, Vec3)> {
    let scene_aabb = static_scene_aabb(scene);
    let nav_aabb = nav.and_then(nav_surface_aabb);
    match (nav_aabb, scene_aabb) {
        (Some((nmin, nmax)), Some((smin, smax))) => {
            let min = nmin.max(smin);
            let max = nmax.min(smax);
            // Keep the nav y-range (actor volume) even if it exceeds the geometry box.
            let min = Vec3::new(min.x, nmin.y, min.z);
            let max = Vec3::new(max.x, nmax.y, max.z);
            (max.cmpgt(min).all()).then_some((min, max))
        }
        (Some(b), None) => Some(b),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Plan the light-probe grid for `scene`. `spacing` is the grid cell size in world
/// units (clamped to a sane minimum). Returns `None` when there is nothing to bound
/// (an empty/static-less scene), in which case the caller places no light probes.
pub fn plan_light_probes(
    scene: &Scene,
    nav: Option<&NavigationGraph>,
    spacing: f32,
) -> Option<ProbeGridPlan> {
    let (min, max) = placement_bounds(scene, nav)?;
    Some(ProbeGridPlan {
        min,
        max,
        spacing: spacing.max(0.5),
    })
}

/// Per-axis region count for a span, so cells stay near `target` units across. At
/// least one region; capped to keep the bake bounded.
fn axis_regions(span: f32, target: f32, cap: u32) -> u32 {
    let target = target.max(0.5);
    ((span / target).round() as u32).clamp(1, cap)
}

/// Plan reflection probes by subdividing the static AABB into a coarse `region`-sized
/// grid of rooms, one probe per cell (centroid + the cell's box). `region` is the
/// approximate room edge length in world units; `cap` bounds the per-axis count so a
/// huge level can't explode the bake. Empty when the scene has no static bounds.
pub fn plan_reflection_probes(scene: &Scene, region: f32, cap: u32) -> Vec<ReflectionPlan> {
    let Some((min, max)) = static_scene_aabb(scene) else {
        return Vec::new();
    };
    let span = (max - min).max(Vec3::splat(0.01));
    let nx = axis_regions(span.x, region, cap);
    // Rooms rarely stack vertically; keep the vertical split shallow.
    let ny = axis_regions(span.y, region, cap.min(2));
    let nz = axis_regions(span.z, region, cap);
    let step = span / Vec3::new(nx as f32, ny as f32, nz as f32);
    let mut plans = Vec::with_capacity((nx * ny * nz) as usize);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let cell = Vec3::new(i as f32, j as f32, k as f32);
                let box_min = min + step * cell;
                let box_max = box_min + step;
                plans.push(ReflectionPlan {
                    position: (box_min + box_max) * 0.5,
                    box_min,
                    box_max,
                });
            }
        }
    }
    plans
}

#[cfg(test)]
#[path = "lighting_placement_tests.rs"]
mod lighting_placement_tests;
