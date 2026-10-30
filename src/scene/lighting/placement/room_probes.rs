//! One reflection probe per room (#1584): its box fitted to the room's walls, floor
//! and ceiling, its capture point at eye height over the room's middle.
//!
//! A room wider than the region is cut into a grid of tiles no larger than it, one
//! probe each. A tile's box spans its cells, grown by the grid's inset to meet the
//! walls; its floor is the lowest under it, its ceiling the median underside over its
//! cells, or `sky` when most of it is open. The capture point stands over the open
//! cell nearest the tile's centroid, so it is never inside a wall, at
//! [`EYE_HEIGHT`] (kept under that cell's own ceiling, and the box raised to hold
//! it).

use glam::{Vec2, Vec3};

use super::floor::FloorGrid;
use super::ReflectionPlan;

/// How high over the floor a probe captures from: a standing actor's eyes.
pub const EYE_HEIGHT: f32 = 1.6;
/// The least room a capture point keeps under the ceiling.
const CEILING_GAP: f32 = 0.1;

/// The probes for the rooms `find_rooms` labelled (`room` of each cell, `count`
/// rooms), in room order and then tile order. `region` is the widest a probe's
/// box may be; `sky` is the height an open room's box reaches.
pub fn plan_rooms(
    grid: &FloorGrid,
    room: &[Option<u32>],
    count: u32,
    region: f32,
    sky: f32,
) -> Vec<ReflectionPlan> {
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); count as usize];
    for (i, r) in room.iter().enumerate() {
        if let Some(r) = r {
            cells[*r as usize].push(i);
        }
    }
    let mut plans = Vec::new();
    for room_cells in &cells {
        for tile in tiles(grid, room_cells, region.max(grid.cell)) {
            plans.push(plan_tile(grid, &tile, sky));
        }
    }
    plans
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

/// The probe for one tile's `cells` (all open, at least one).
fn plan_tile(grid: &FloorGrid, cells: &[usize], sky: f32) -> ReflectionPlan {
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
    let ceiling = ceiling(cells.iter().map(|&i| open(i).ceiling), sky).max(floor + EYE_HEIGHT);
    let centroid = sum / cells.len() as f32;
    let stand = (cells.iter().copied())
        .min_by(|&a, &b| {
            (centre(a).distance_squared(centroid)).total_cmp(&centre(b).distance_squared(centroid))
        })
        .expect("a tile has cells");
    // Under the standing cell's own ceiling, not the tile's: on a stair the median
    // can sit below the step it stands on.
    let own = open(stand);
    let eye = (own.floor + EYE_HEIGHT).min(own.ceiling - CEILING_GAP);
    let at = centre(stand);
    ReflectionPlan {
        position: Vec3::new(at.x, eye, at.y),
        box_min: Vec3::new(lo.x, floor, lo.y),
        box_max: Vec3::new(hi.x, ceiling.max(eye + CEILING_GAP), hi.y),
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
