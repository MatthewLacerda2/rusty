use glam::Vec3;

use super::enclosure::CLUTTER_GAIN;
use super::grid::static_scene_aabb;
use super::room_probes::{plan_rooms, Rooms};
use super::rooms::{find_rooms, RoomParams};
use super::{ReflectionPlan, Site, VERTICAL_HEADROOM};
use rusty_scene::lighting::settings::ReflectionSettings;
use rusty_scene::{ReflectionProbe, Scene};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionPlacement {
    pub region: f32,
    pub cell: f32,
    pub rooms: RoomParams,
    pub max_probes: usize,
}

/// World units: below it the probe count grows with the square.
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
    let (mut region, (mut plans, mut single)) = (asked, plan(asked));
    // Clutter divides a region by at most 1 + CLUTTER_GAIN (`cluttered_region`): past this
    // bound every room is one tile.
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

fn covers(m: &ReflectionProbe, plan: &ReflectionPlan) -> bool {
    match (plan.box_max - plan.box_min).cmpgt(Vec3::ZERO).all() {
        true => m.share_of(plan.box_min, plan.box_max) >= 0.5,
        false => m.contains(plan.position),
    }
}

fn keep_largest(plans: &mut Vec<ReflectionPlan>, max: usize) {
    let area = |p: &ReflectionPlan| (p.box_max - p.box_min).max(Vec3::ZERO).element_product();
    let mut order: Vec<usize> = (0..plans.len()).collect();
    order.sort_by(|&a, &b| area(&plans[b]).total_cmp(&area(&plans[a])).then(a.cmp(&b)));
    let mut keep = vec![false; plans.len()];
    order.into_iter().take(max).for_each(|i| keep[i] = true);
    let mut kept = keep.into_iter();
    plans.retain(|_| kept.next().unwrap_or(false));
}
