//! One reflection probe per room (#1584): its box fitted to the room's walls, floor
//! and ceiling, its capture point at eye height over the room's middle.
//!
//! A room wider than its region is cut into a grid of tiles no larger than it, one
//! probe each; the region narrows with the room's clutter (`enclosure.rs`, #1737).
//! A tile's box spans its cells, grown by the grid's inset to meet the walls; its
//! floor is the lowest under it, its ceiling the median underside over its cells
//! (or `sky` when most of it is open), lowered to the first solid straight above
//! the capture point. The capture point stands over the open cell nearest the
//! tile's centroid that keeps clear of geometry, so it is never inside a wall, at
//! [`EYE_HEIGHT`] (kept under that cell's own ceiling, and the box raised to hold
//! it). A room most of whose tiles see the sky is open air: one probe over all of
//! it, without box projection, at an importance under the room probes', since its
//! box holds every room inside its footprint.

use glam::{Vec2, Vec3};

use super::enclosure::CAPTURE_CLEARANCE;
use super::enclosure::{ceiling_over, clearest, cluttered_region, open_to_sky, room_clutter};
use super::occupancy::Occupancy;
use super::{ReflectionPlan, MIN_REGION, OPEN_AIR_IMPORTANCE};
use rusty_scene::lighting::floor::FloorGrid;
use rusty_scene::ReflectionProbe;

/// How high over the floor a probe captures from: a standing actor's eyes.
pub const EYE_HEIGHT: f32 = 1.6;
/// The least room a capture point keeps under the ceiling.
const CEILING_GAP: f32 = 0.1;

/// The rooms `find_rooms` labelled, each with its cells and its clutter. The
/// clutter never depends on the region, so a placement that re-tiles wider reads
/// it once per room, not once per pass.
pub struct Rooms {
    cells: Vec<Vec<usize>>,
    clutter: Vec<f32>,
}

impl Rooms {
    /// The `count` rooms of `room` (each cell's room) on `site`.
    pub fn new(site: (&FloorGrid, &Occupancy), room: &[Option<u32>], count: u32) -> Self {
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); count as usize];
        for (i, r) in room.iter().enumerate() {
            if let Some(r) = r {
                cells[*r as usize].push(i);
            }
        }
        let clutter = (cells.iter())
            .map(|c| room_clutter(site.0, site.1, c))
            .collect();
        Self { cells, clutter }
    }
}

/// The probes for `rooms`, in room order and then tile order, and whether every
/// room is one tile (so a wider region places no fewer). `region` is the widest
/// an empty room's probe box may be; `sky` is the height an open room's box reaches.
pub fn plan_rooms(
    site: (&FloorGrid, &Occupancy),
    rooms: &Rooms,
    region: f32,
    sky: f32,
) -> (Vec<ReflectionPlan>, bool) {
    let (grid, occ) = site;
    let mut plans = Vec::new();
    let mut single = true;
    for (room_cells, &clutter) in rooms.cells.iter().zip(&rooms.clutter) {
        let region = cluttered_region(region, clutter, MIN_REGION);
        let tiles: Vec<ReflectionPlan> = (tiles(grid, room_cells, region.max(grid.cell)).iter())
            .map(|tile| plan_tile(site, tile, sky, false))
            .collect();
        single &= tiles.len() <= 1;
        let open = tiles
            .iter()
            .filter(|p| open_to_sky(occ, p.position))
            .count();
        match open * 2 > tiles.len() {
            true => plans.push(ReflectionPlan {
                box_projection: false,
                importance: OPEN_AIR_IMPORTANCE,
                ..plan_tile(site, room_cells, sky, true)
            }),
            false => plans.extend(tiles),
        }
    }
    (plans, single)
}

/// `cells` cut into a grid of tiles at most `region` across, non-empty ones only,
/// row-major.
fn tiles(grid: &FloorGrid, cells: &[usize], region: f32) -> Vec<Vec<usize>> {
    let xz = |i: usize| (i % grid.width, i / grid.width);
    let (mut lo, mut hi) = ((usize::MAX, usize::MAX), (0, 0));
    for &i in cells {
        let (x, z) = xz(i);
        (lo, hi) = ((lo.0.min(x), lo.1.min(z)), (hi.0.max(x), hi.1.max(z)));
    }
    let along = |a: usize, b: usize| (b - a + 1, ((b - a + 1) as f32 * grid.cell / region).ceil());
    let ((wide, nx), (deep, nz)) = (along(lo.0, hi.0), along(lo.1, hi.1));
    let (nx, nz) = (nx.max(1.0) as usize, nz.max(1.0) as usize);
    let mut out = vec![Vec::new(); nx * nz];
    for &i in cells {
        let (x, z) = xz(i);
        let tx = (x - lo.0) * nx / wide;
        let tz = (z - lo.1) * nz / deep;
        out[tz * nx + tx].push(i);
    }
    out.retain(|t| !t.is_empty());
    out
}

/// The probe for one tile's `cells` (all open, at least one). An `open_air` probe
/// captures from the first point near the centroid that is itself open to the sky
/// and clear: a yard round a house has its centroid inside the house, and the
/// nearest clear point to it, the doorway, would show the yard the room's walls.
fn plan_tile(
    (grid, occ): (&FloorGrid, &Occupancy),
    cells: &[usize],
    sky: f32,
    open_air: bool,
) -> ReflectionPlan {
    let centre = |i: usize| grid.centre(i % grid.width, i / grid.width);
    let open = |i: usize| grid.cells[i].expect("a room's cells are open");
    let half = Vec2::splat(grid.cell * 0.5 + grid.inset);
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    let mut floor = f32::MAX;
    let mut sum = Vec2::ZERO;
    for &i in cells {
        let c = centre(i);
        (lo, hi) = (lo.min(c - half), hi.max(c + half));
        floor = floor.min(open(i).floor);
        sum += c;
    }
    // Never past the plan: a room on the level's edge stops at it.
    let first = grid.origin - Vec2::splat(grid.cell * 0.5);
    let last = grid.centre(grid.width - 1, grid.depth - 1) + Vec2::splat(grid.cell * 0.5);
    (lo, hi) = (lo.max(first), hi.min(last));
    let centroid = sum / cells.len() as f32;
    let mut by_centre: Vec<usize> = cells.to_vec();
    by_centre.sort_by(|&a, &b| {
        (centre(a).distance_squared(centroid)).total_cmp(&centre(b).distance_squared(centroid))
    });
    // Under the standing cell's own ceiling, not the tile's: on a stair the median
    // can sit below the step it stands on.
    let eye = |i: usize| {
        let (own, at) = (open(i), centre(i));
        let y = (own.floor + EYE_HEIGHT).min(own.ceiling - CEILING_GAP);
        Vec3::new(at.x, y, at.y)
    };
    let candidates: Vec<(usize, Vec3)> = by_centre.iter().map(|&i| (i, eye(i))).collect();
    let outdoors = |&(_, at): &(usize, Vec3)| {
        open_air && occ.clearance(at) >= CAPTURE_CLEARANCE && open_to_sky(occ, at)
    };
    let (_, position) = (candidates.iter().copied().find(outdoors))
        .or_else(|| clearest(occ, &candidates))
        .expect("a tile has cells");
    let floor_plan = ceiling(cells.iter().map(|&i| open(i).ceiling), sky);
    // The ray is good to a cell: it lowers the floor plan's ceiling only past that.
    let ray = ceiling_over(occ, position).filter(|&c| c < floor_plan - occ.cell);
    let ceiling = ray.unwrap_or(floor_plan).max(floor + EYE_HEIGHT);
    ReflectionPlan {
        position,
        box_min: Vec3::new(lo.x, floor, lo.y),
        box_max: Vec3::new(hi.x, ceiling.max(position.y + CEILING_GAP), hi.y),
        box_projection: true,
        importance: ReflectionProbe::default().importance,
    }
}

/// A tile's ceiling: the median underside over its cells, or `sky` when most of
/// them are open to it.
fn ceiling(undersides: impl Iterator<Item = f32>, sky: f32) -> f32 {
    let mut all: Vec<f32> = undersides.collect();
    let covered = all.iter().filter(|c| c.is_finite()).count();
    if covered * 2 <= all.len() {
        return sky;
    }
    all.sort_by(f32::total_cmp);
    all[all.len() / 2]
}
