//! What the occupancy grid tells reflection placement about a room (#1737).
//!
//! * **Open sky.** Rays cast up from a capture point (straight up, and a ring at
//!   [`RING_TILT`] from vertical) either hit a ceiling or leave the level. A room
//!   where most of them leave is open air (a courtyard, a pool hall): box
//!   projection against a ceiling that isn't there bends the sky, so it gets one
//!   probe without it.
//! * **Ceiling.** Under a roof, the box's top is where the straight-up ray hits,
//!   when that is lower than the floor plan says (a glass roof, a lean-to).
//! * **Clutter.** The solid fraction of the air a room's actors move through
//!   (between a step and head height over its floor, column by column, the small
//!   obstacles it encloses included) sets how dense its probes are: a hangar full
//!   of crates gets more and smaller boxes than an empty hall, and a corridor round
//!   a solid core no more than an empty corridor.
//! * **Clearance.** A capture point keeps [`CAPTURE_CLEARANCE`] from geometry,
//!   so it never sits in a pillar or against a wall.

use glam::{Vec2, Vec3};

use rusty_scene::lighting::floor::FloorGrid;
use super::occupancy::Occupancy;
use rusty_core::collections::Set;

/// How far up a ray looks for a ceiling, world units: past it is sky.
const SKY_REACH: f32 = 60.0;
/// The tilt of the ring of up rays from vertical, radians (30°).
const RING_TILT: f32 = std::f32::consts::FRAC_PI_6;
/// Rays in the ring.
const RING: usize = 8;
/// How much more densely a room is probed per unit of clutter: a room a quarter
/// full of geometry takes probes four times as narrow as an empty one.
pub const CLUTTER_GAIN: f32 = 12.0;
/// The least distance a capture point keeps from geometry, world units.
pub const CAPTURE_CLEARANCE: f32 = 0.5;
/// The body layer clutter is measured in: from a step over the floor to a head.
const BODY: (f32, f32) = (0.4, 1.8);
/// The largest blocked patch a room counts as clutter, square metres: a vehicle,
/// a stack of crates. A larger one is architecture.
pub const CLUTTER_PATCH: f32 = 16.0;

/// Whether most of the up rays from `at` leave the level without a hit.
pub fn open_to_sky(occ: &Occupancy, at: Vec3) -> bool {
    let (sin, cos) = RING_TILT.sin_cos();
    let ring = (0..RING).map(|k| {
        let a = k as f32 / RING as f32 * std::f32::consts::TAU;
        Vec3::new(a.cos() * sin, cos, a.sin() * sin)
    });
    let rays: Vec<Vec3> = std::iter::once(Vec3::Y).chain(ring).collect();
    let open = (rays.iter())
        .filter(|&&d| occ.ray(at, d, SKY_REACH).is_none())
        .count();
    open * 2 > rays.len()
}

/// The world `y` of the first solid straight above `at`, if one is in reach.
pub fn ceiling_over(occ: &Occupancy, at: Vec3) -> Option<f32> {
    occ.ray(at, Vec3::Y, SKY_REACH).map(|t| at.y + t)
}

/// The solid fraction of the body layer over a room (`cells` of `grid`): its own
/// columns, each from its own floor, and the small blocked patches it encloses
/// (crates, a vehicle: no larger than [`CLUTTER_PATCH`]), each from the lowest
/// floor beside it. A larger enclosed patch is architecture, the solid core a
/// corridor wraps, not clutter; so is everything outside the room.
pub fn room_clutter(grid: &FloorGrid, occ: &Occupancy, cells: &[usize]) -> f32 {
    let mut columns: Vec<(usize, f32)> = (cells.iter())
        .filter_map(|&i| grid.cells[i].map(|c| (i, c.floor)))
        .collect();
    columns.extend(enclosed_patches(grid, cells));
    let per = (grid.cell / occ.cell).round().max(1.0) as usize;
    let (mut solid, mut all) = (0usize, 0usize);
    for &(i, floor) in &columns {
        let corner = grid.centre(i % grid.width, i / grid.width) - Vec2::splat(grid.cell * 0.5);
        for (a, b) in (0..per).flat_map(|a| (0..per).map(move |b| (a, b))) {
            let at = corner + (Vec2::new(a as f32, b as f32) + 0.5) * (grid.cell / per as f32);
            let mut y = floor + BODY.0 + occ.cell * 0.5;
            while y < floor + BODY.1 {
                all += 1;
                solid += usize::from(occ.is_solid(Vec3::new(at.x, y, at.y)));
                y += occ.cell;
            }
        }
    }
    match all {
        0 => 0.0,
        _ => solid as f32 / all as f32,
    }
}

/// The blocked cells `cells` encloses in patches of at most [`CLUTTER_PATCH`],
/// each with the lowest floor of the room cells beside its patch. The search
/// stays inside the room's bounding rectangle, so it costs the room's area.
fn enclosed_patches(grid: &FloorGrid, cells: &[usize]) -> Vec<(usize, f32)> {
    let mut mine = Set::default();
    mine.extend(cells.iter().copied());
    let xz = |i: usize| (i % grid.width, i / grid.width);
    let (mut lo, mut hi) = ((usize::MAX, usize::MAX), (0, 0));
    for (x, z) in cells.iter().map(|&i| xz(i)) {
        (lo, hi) = ((lo.0.min(x), lo.1.min(z)), (hi.0.max(x), hi.1.max(z)));
    }
    let inside = |n: Option<usize>| {
        n.filter(|&n| {
            let (x, z) = xz(n);
            (lo.0..=hi.0).contains(&x) && (lo.1..=hi.1).contains(&z)
        })
    };
    let most = (CLUTTER_PATCH / (grid.cell * grid.cell)).ceil() as usize;
    let (mut seen, mut out) = (Set::default(), Vec::new());
    for &start in cells {
        for next in neighbours(grid, start).into_iter().filter_map(inside) {
            if grid.cells[next].is_some() || !seen.insert(next) {
                continue;
            }
            let (mut patch, mut queue, mut floor) = (Vec::new(), vec![next], f32::MAX);
            let mut enclosed = true;
            while let Some(i) = queue.pop() {
                patch.push(i);
                enclosed &= patch.len() <= most;
                for n in neighbours(grid, i).map(inside) {
                    match n {
                        None => enclosed = false, // past the room's rectangle
                        Some(n) if grid.cells[n].is_none() => {
                            if seen.insert(n) {
                                queue.push(n);
                            }
                        }
                        Some(n) if mine.contains(&n) => {
                            floor = floor.min(grid.cells[n].map_or(f32::MAX, |c| c.floor));
                        }
                        Some(_) => enclosed = false, // another room's floor
                    }
                }
            }
            if enclosed {
                patch.sort_unstable();
                out.extend(patch.into_iter().map(|i| (i, floor)));
            }
        }
    }
    out
}

/// The four cells beside `i`, `None` past the plan's edge.
fn neighbours(grid: &FloorGrid, i: usize) -> [Option<usize>; 4] {
    let (x, z) = (i % grid.width, i / grid.width);
    [
        (x > 0).then(|| i - 1),
        (x + 1 < grid.width).then(|| i + 1),
        (z > 0).then(|| i - grid.width),
        (z + 1 < grid.depth).then(|| i + grid.width),
    ]
}

/// The probe width for a room of `clutter` when an empty one takes `region`,
/// never under `least`.
pub fn cluttered_region(region: f32, clutter: f32, least: f32) -> f32 {
    (region / (1.0 + CLUTTER_GAIN * clutter.max(0.0))).max(least.min(region))
}

/// The first of `candidates` whose point keeps [`CAPTURE_CLEARANCE`], else the
/// one with the most room around it; ties keep the earlier.
pub fn clearest<T: Copy>(occ: &Occupancy, candidates: &[(T, Vec3)]) -> Option<(T, Vec3)> {
    let mut best: Option<(f32, (T, Vec3))> = None;
    for &(id, at) in candidates {
        let room = occ.clearance(at);
        if room >= CAPTURE_CLEARANCE {
            return Some((id, at));
        }
        if best.is_none_or(|(r, _)| room > r) {
            best = Some((room, (id, at)));
        }
    }
    best.map(|(_, c)| c)
}
