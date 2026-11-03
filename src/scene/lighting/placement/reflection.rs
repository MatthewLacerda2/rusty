//! Reflection probes, one per room (#1584): the floor plan cut into rooms at its
//! doorways (`rooms.rs`), each room's probes fitted to it (`room_probes.rs`), and
//! the rooms a manual probe already covers left to it (#1737).

use super::grid::static_scene_aabb;
use super::room_probes::plan_rooms;
use super::rooms::{find_rooms, RoomParams};
use super::{ReflectionPlan, Site, VERTICAL_HEADROOM};
use crate::scene::lighting::settings::ReflectionSettings;
use crate::scene::{ReflectionProbe, Scene};

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

/// Plan one reflection probe per room of `site`'s floor plan (#1584). Each probe's
/// box fits its room's walls, floor and ceiling (an open room reaches the top of
/// the static geometry), and it captures from eye height over the room's middle;
/// a room open to the sky gets one probe without box projection (#1737); see
/// `room_probes.rs`. A planned probe whose capture point is inside a `manual`
/// probe's box is left out: that room is the manual probe's. Empty with nothing
/// to stand on.
///
/// Bounded: `region` is held to at least [`MIN_REGION`], and a level that would
/// take more than `max_probes` tiles is re-tiled wider, with a warning, until it
/// fits or every room is one tile.
pub fn plan_reflection_probes(
    scene: &Scene,
    site: &Site,
    params: &ReflectionPlacement,
    manual: &[ReflectionProbe],
) -> Vec<ReflectionPlan> {
    let grid = &site.floor;
    let Some(highest_floor) = (grid.cells.iter().flatten())
        .map(|c| c.floor)
        .reduce(f32::max)
    else {
        return Vec::new();
    };
    let sky = static_scene_aabb(scene)
        .map_or(f32::MIN, |(_, max)| max.y)
        .max(highest_floor + VERTICAL_HEADROOM);
    let (room, count) = find_rooms(grid, &params.rooms);
    let asked = params.region.max(MIN_REGION);
    let widest = grid.cell * grid.width.max(grid.depth) as f32;
    let max = params.max_probes.saturating_sub(manual.len()).max(1);
    let plan = |region: f32| {
        let mut plans = plan_rooms((grid, &site.occupancy), &room, count, region, sky);
        plans.retain(|p| !manual.iter().any(|m| m.contains(p.position)));
        plans
    };
    let (mut region, mut plans) = (asked, plan(asked));
    while plans.len() > max && region < widest {
        region *= (plans.len() as f32 / max as f32).sqrt().max(1.1);
        plans = plan(region);
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
