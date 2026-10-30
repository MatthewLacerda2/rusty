//! src/scene/lighting/placement.rs — Deterministic auto-placement of IBL probes.
//!
//! The "place well" half of the one-button bake (#246): given the STATIC scene (and,
//! when available, the walkable extent of the baked nav surface), compute where light
//! probes and reflection probes should sit. This is a PURE, deterministic function of
//! the scene and those bounds —
//! a grid and a floor-plan watershed, no RNG and no wall-clock — so the same level always
//! yields the same probe layout (the determinism the headless bake relies on).
//!
//! The bake itself (rendering each probe → SH / prefiltered cubemap) is the render/dev
//! half and lives in `dev::bake::lighting`; this module only decides POSITIONS + bounds.
//!
//! Two surfaces are placed:
//!   * **Light probes** — a regular grid through the navigable volume. Bounds come from
//!     the walkable extent the caller passes in (`NavigationGraph::walkable_aabb`,
//!     raised through a vertical headroom so actors at different elevations get
//!     coverage) when a nav surface is baked; otherwise they fall back to the
//!     static-geometry AABB. Taking bounds, not the graph, keeps the arrow
//!     navigation → scene (#721).
//!   * **Reflection probes** — one per room (#1584): the floor plan (the nav
//!     surface's, handed over as data, else rasterized from the static colliders)
//!     is cut into rooms at its doorways (`rooms.rs`), and each room gets a probe
//!     at eye height with its box fitted to the room (`room_probes.rs`).
//!
//! This module is pure data + math — deterministic, sim-reachable, no RNG/clock.

mod floor;
mod room_probes;
mod rooms;
#[cfg(test)]
pub(crate) mod test_level;

use glam::Vec3;

pub use self::floor::{FloorCell, FloorGrid};
use self::room_probes::plan_rooms;
pub use self::room_probes::EYE_HEIGHT;
use self::rooms::find_rooms;
pub use self::rooms::RoomParams;
use crate::scene::lighting::settings::ReflectionSettings;
use crate::scene::Scene;

/// A light-probe grid request: the inclusive `[min, max]` corners and the cell spacing,
/// ready to hand to [`crate::scene::lighting::probe::ProbeVolume::fill_grid`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeGridPlan {
    pub min: Vec3,
    pub max: Vec3,
    pub spacing: f32,
}

/// A reflection-probe request: capture position (eye height over the room's middle)
/// plus the parallax box corners fitted to the room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionPlan {
    pub position: Vec3,
    pub box_min: Vec3,
    pub box_max: Vec3,
}

/// Vertical headroom added above the placement bounds so probes also cover the air an
/// actor's head/torso occupies, not only the floor it stands on. World units.
const VERTICAL_HEADROOM: f32 = 3.0;

/// World-space AABB of the scene's STATIC, active, solid geometry (its colliders;
/// a trigger is not geometry). `None`
/// when the scene has no static collider to bound — the caller then has nothing to
/// place against. Deterministic: a pure fold over the entities in id order.
pub fn static_scene_aabb(scene: &Scene) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;
    for id in scene.world.ids_with_collider() {
        if !scene.world.is_static(id) || !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let collider = scene
            .world
            .collider(id)
            .expect("id came from ids_with_collider");
        if !collider.active || collider.is_trigger {
            continue;
        }
        let Some(pose) = scene.world.world_pose(id) else {
            continue;
        };
        let (cmin, cmax) = collider.calculate_world_aabb(pose);
        min = min.min(cmin);
        max = max.max(cmax);
        found = true;
    }
    found.then_some((min, max))
}

/// The volume actors move through above a walkable extent: its floors, raised by
/// `VERTICAL_HEADROOM` so probes cover heads and torsos, not only feet.
fn actor_volume((min, mut max): (Vec3, Vec3)) -> (Vec3, Vec3) {
    max.y += VERTICAL_HEADROOM;
    (min, max)
}

/// Choose the light-probe grid bounds: the navigable volume intersected with the
/// scene's static AABB when walkable bounds are given, else the static AABB alone,
/// else `None` (nothing to place). The intersection keeps the grid inside the level
/// rather than spilling past its walls when the nav extent is larger.
fn placement_bounds(scene: &Scene, walkable: Option<(Vec3, Vec3)>) -> Option<(Vec3, Vec3)> {
    let scene_aabb = static_scene_aabb(scene);
    let nav_aabb = walkable.map(actor_volume);
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

/// Plan the light-probe grid for `scene`. `walkable` is the `[min, max]` box of the
/// spans agents stand on (`NavigationGraph::walkable_aabb`), `None` without a baked
/// nav surface. `spacing` is the grid cell size in world units (clamped to a sane
/// minimum). Returns `None` when there is nothing to bound (an empty/static-less
/// scene), in which case the caller places no light probes.
pub fn plan_light_probes(
    scene: &Scene,
    walkable: Option<(Vec3, Vec3)>,
    spacing: f32,
) -> Option<ProbeGridPlan> {
    let (min, max) = placement_bounds(scene, walkable)?;
    Some(ProbeGridPlan {
        min,
        max,
        spacing: spacing.max(0.5),
    })
}

/// How reflection probes are placed: one per room, a room larger than `region`
/// split into probes no wider than it, at most `max_probes` in all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionPlacement {
    /// The widest a probe's box may be, world units (`reflectionRegion`), at least
    /// [`MIN_REGION`].
    pub region: f32,
    /// The floor plan's cell when it is rasterized from the colliders.
    pub cell: f32,
    /// What separates two rooms.
    pub rooms: RoomParams,
    /// The most probes placed (`LightingSettings::reflections`); over it the tiles
    /// widen until the level fits.
    pub max_probes: usize,
}

/// The narrowest `region` placement accepts, world units: a probe per two metres
/// already over-samples a room, and below it the count grows with the square.
pub const MIN_REGION: f32 = 2.0;

impl Default for ReflectionPlacement {
    fn default() -> Self {
        Self {
            region: 12.0,
            cell: 0.5,
            rooms: RoomParams::default(),
            max_probes: ReflectionSettings::default().max_probes as usize,
        }
    }
}

/// Plan one reflection probe per room of `scene` (#1584): the rooms of `floor`, the
/// nav surface's floor plan (`NavigationGraph::floor_grid`), or with none the plan
/// [`FloorGrid::from_static`] rasterizes from the static colliders. Each probe's box
/// fits its room's walls, floor and ceiling (an open room reaches the top of the
/// static geometry), and it captures from eye height over the room's middle; see
/// `rooms.rs` and `room_probes.rs`. Empty with nothing to stand on.
///
/// Bounded: `region` is held to at least [`MIN_REGION`], and a level that would
/// take more than `max_probes` tiles is re-tiled wider, with a warning, until it
/// fits or every room is one tile.
pub fn plan_reflection_probes(
    scene: &Scene,
    floor: Option<FloorGrid>,
    params: &ReflectionPlacement,
) -> Vec<ReflectionPlan> {
    let Some(grid) = floor.or_else(|| FloorGrid::from_static(scene, params.cell)) else {
        return Vec::new();
    };
    let highest_floor = (grid.cells.iter().flatten())
        .map(|c| c.floor)
        .fold(f32::MIN, f32::max);
    let sky = static_scene_aabb(scene)
        .map_or(f32::MIN, |(_, max)| max.y)
        .max(highest_floor + VERTICAL_HEADROOM);
    let (room, count) = find_rooms(&grid, &params.rooms);
    let asked = params.region.max(MIN_REGION);
    let widest = grid.cell * grid.width.max(grid.depth) as f32;
    let max = params.max_probes.max(1);
    let (mut region, mut plans) = (asked, plan_rooms(&grid, &room, count, asked, sky));
    while plans.len() > max && region < widest {
        region *= (plans.len() as f32 / max as f32).sqrt().max(1.1);
        plans = plan_rooms(&grid, &room, count, region, sky);
    }
    if region > asked {
        log::warn!(
            "[Lighting] {asked} m reflection tiles pass the {max}-probe ceiling; \
             placed {} probes on {region:.1} m tiles",
            plans.len()
        );
    }
    plans
}

#[cfg(test)]
#[path = "../placement_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "limits_tests.rs"]
mod limits_tests;
