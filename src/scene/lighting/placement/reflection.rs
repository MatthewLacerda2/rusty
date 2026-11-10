//! Reflection probes, one per room (#1584): the floor plan cut into rooms at its
//! doorways (`rooms.rs`), each room's probes fitted to it (`room_probes.rs`), and
//! the rooms a manual probe already covers left to it (#1737).

use glam::Vec3;

use super::enclosure::CLUTTER_GAIN;
use super::grid::static_scene_aabb;
use super::room_probes::{plan_rooms, Rooms};
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
/// `room_probes.rs`. A planned probe whose box a `manual` probe's box covers by at
/// least half is left out: that room is the manual probe's. Empty with nothing
/// to stand on.
///
/// Bounded: `region` is held to at least [`MIN_REGION`], and a level that would
/// take more than `max_probes` probes, the manual ones counted, is re-tiled wider,
/// with a warning, until it fits or every room is one tile; past that the largest
/// rooms keep theirs. So no more than `max_probes` are ever placed, and none when
/// the manual probes already reach it.
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
    // The manual probes count against the ceiling: with none left, none is placed.
    let max = params.max_probes.saturating_sub(manual.len());
    if max == 0 {
        log::warn!(
            "[Lighting] the manual reflection probes fill the {}-probe ceiling; placed none",
            params.max_probes
        );
        return Vec::new();
    }
    let site_ref = (grid, &site.occupancy);
    let rooms = Rooms::new(site_ref, &room, count);
    let plan = |region: f32| {
        let (mut plans, single) = plan_rooms(site_ref, &rooms, region, sky);
        plans.retain(|p| !manual.iter().any(|m| covers(m, p)));
        (plans, single)
    };
    // Clutter narrows a room's tiles under the region (`enclosure.rs`), so the region
    // widens until even the most cluttered room is one tile; past that a wider one
    // places no fewer, so the loop stops.
    let (mut region, (mut plans, mut single)) = (asked, plan(asked));
    while plans.len() > max && !single && region < widest * (1.0 + CLUTTER_GAIN) {
        region *= (plans.len() as f32 / max as f32).sqrt().max(1.1);
        (plans, single) = plan(region);
    }
    let cut = plans.len() > max;
    if cut {
        keep_largest(&mut plans, max);
    }
    if region > asked || cut {
        log::warn!(
            "[Lighting] {asked} m reflection tiles pass the {max}-probe ceiling; \
             placed {} probes on {region:.1} m tiles",
            plans.len()
        );
    }
    plans
}

/// Whether manual probe `m` takes `plan`'s place: its box covers at least half of
/// the plan's box (#1737). A small manual probe in a corner of a room leaves the
/// room its own probe. An oriented box or a sphere (#1995) is judged by its
/// world-aligned bounds.
fn covers(m: &ReflectionProbe, plan: &ReflectionPlan) -> bool {
    let volume = |lo: Vec3, hi: Vec3| (hi - lo).max(Vec3::ZERO).element_product();
    let own = volume(plan.box_min, plan.box_max);
    let (lo, hi) = m.world_bounds();
    let shared = volume(lo.max(plan.box_min), hi.min(plan.box_max));
    match own > 0.0 {
        true => shared >= own * 0.5,
        false => m.contains(plan.position),
    }
}

/// Keep the `max` plans with the largest boxes (more rooms than the ceiling, each
/// already one tile), in their order; ties keep the earlier.
fn keep_largest(plans: &mut Vec<ReflectionPlan>, max: usize) {
    let area = |p: &ReflectionPlan| (p.box_max - p.box_min).max(Vec3::ZERO).element_product();
    let mut order: Vec<usize> = (0..plans.len()).collect();
    order.sort_by(|&a, &b| area(&plans[b]).total_cmp(&area(&plans[a])).then(a.cmp(&b)));
    let mut keep = vec![false; plans.len()];
    order.into_iter().take(max).for_each(|i| keep[i] = true);
    let mut kept = keep.into_iter();
    plans.retain(|_| kept.next().unwrap_or(false));
}
