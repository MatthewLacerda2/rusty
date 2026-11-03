//! Light probes from the occupancy grid (#1737), after Unity's Adaptive Probe
//! Volumes: denser near geometry, sparse in open air, only where actors are.
//!
//! * **Where.** The air over the floor plan's floors (every storey, `upper`
//!   included) from [`LOWEST`] to [`VERTICAL_HEADROOM`](super::VERTICAL_HEADROOM)
//!   up, never past a cell's ceiling, never through a solid between the floor and
//!   the probe. So nothing is placed inside a wall, above a ceiling, or over floor
//!   no actor reaches (the floor plan holds only reachable floor when a nav
//!   surface is baked).
//! * **How dense.** A lattice at half the spacing, anchored to the world in XZ
//!   and to each floor in height. Its every other point (the coarse lattice at the
//!   full spacing) is always kept; the points between are kept only near
//!   geometry: a horizontal ray from them meets a wall, a pillar or a crate within
//!   half the spacing.
//! * **Virtual offset.** A probe closer than [`PROBE_CLEARANCE`] to geometry is
//!   pushed out along the distance field, at most half a fine step; one still
//!   inside a solid is dropped (Unity marks it invalid).
//! * **Manual probes** stay where they are; a placed probe nearer one than half
//!   its own step is left out.
//!
//! The result has no regular grid, so it is sampled by blending the nearest
//! probes (`ProbeVolume::sample`). Deterministic: lattice order, no RNG.

use glam::{IVec3, Vec2, Vec3};

use super::floor::FloorGrid;
use super::occupancy::Occupancy;
use super::VERTICAL_HEADROOM;

/// The lowest a probe sits over its floor, world units: a crouching actor's chest.
const LOWEST: f32 = 1.0;
/// The least room a probe keeps from geometry after its virtual offset.
pub const PROBE_CLEARANCE: f32 = 0.3;

/// The light probes for `floor` in `occ` at `spacing` (the open-air step; half of
/// it near geometry), planned around the `manual` probes, in lattice order.
pub fn plan_light_set(
    floor: &FloorGrid,
    occ: &Occupancy,
    spacing: f32,
    manual: &[Vec3],
) -> Vec<Vec3> {
    let fine = spacing.max(1.0) * 0.5;
    let mut points = lattice(floor, fine);
    // One point per lattice key: the lowest floor's, in key order.
    points.sort_by(|a, b| {
        ((a.key.z, a.key.y, a.key.x).cmp(&(b.key.z, b.key.y, b.key.x)))
            .then(a.floor.total_cmp(&b.floor))
    });
    points.dedup_by_key(|p| p.key);
    let mut out = Vec::new();
    for Point {
        key,
        at,
        floor,
        level,
    } in points
    {
        let coarse = key.x % 2 == 0 && key.z % 2 == 0 && level % 2 == 0;
        if !coarse && !near_geometry(occ, at, fine) {
            continue;
        }
        let Some(at) = offset(occ, at, fine * 0.5) else {
            continue;
        };
        // A solid between the floor and the probe: it is above a ceiling.
        let down = at.y - floor - occ.cell * 1.5;
        if down > 0.0 && occ.ray(at, Vec3::NEG_Y, down).is_some() {
            continue;
        }
        let step = if coarse { spacing } else { fine };
        if manual.iter().any(|m| m.distance(at) < step * 0.5) {
            continue;
        }
        out.push(at);
    }
    out
}

/// A candidate probe: its lattice key, where it is, the floor it stands over and
/// its level over that floor (0 at [`LOWEST`]).
struct Point {
    key: IVec3,
    at: Vec3,
    floor: f32,
    level: i32,
}

/// Every lattice point at `fine` over the floors of `floor`.
fn lattice(floor: &FloorGrid, fine: f32) -> Vec<Point> {
    let mut points = Vec::new();
    let floors = (floor.cells.iter().enumerate())
        .filter_map(|(i, c)| c.map(|c| (i, c.floor, c.ceiling)))
        .chain(floor.upper.iter().map(|&(i, y)| (i, y, f32::INFINITY)));
    // A floor's air reaches past its cell by the plan's inset (the nav surface
    // stops an agent radius short of the walls) and a cell, so the air beside a
    // wall, where probes are densest, is covered too.
    let reach = Vec2::splat(floor.cell * 1.5 + floor.inset);
    for (cell, y, ceiling) in floors {
        let at = floor.centre(cell % floor.width, cell / floor.width);
        // The lattice columns within `reach` of this cell's centre: `[lo, hi)`.
        let (lo, hi) = (
            ((at - reach) / fine).ceil(),
            ((at + reach) / fine).floor() + Vec2::ONE,
        );
        let top = (y + VERTICAL_HEADROOM).min(ceiling - PROBE_CLEARANCE);
        let base = ((y + LOWEST) / fine).round() as i32;
        for iz in lo.y as i32..hi.y as i32 {
            for ix in lo.x as i32..hi.x as i32 {
                let mut level = 0;
                while y + LOWEST + level as f32 * fine <= top {
                    points.push(Point {
                        key: IVec3::new(ix, base + level, iz),
                        at: Vec3::new(ix as f32, 0.0, iz as f32) * fine
                            + Vec3::Y * (y + LOWEST + level as f32 * fine),
                        floor: y,
                        level,
                    });
                    level += 1;
                }
            }
        }
    }
    points
}

/// Whether a horizontal ray from `p` meets geometry within `reach`.
fn near_geometry(occ: &Occupancy, p: Vec3, reach: f32) -> bool {
    [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
        .iter()
        .any(|&d| occ.ray(p, d, reach).is_some())
}

/// `p` pushed out of geometry to [`PROBE_CLEARANCE`], moving at most `most`
/// along the distance field's gradient; `None` when it stays inside a solid.
fn offset(occ: &Occupancy, p: Vec3, most: f32) -> Option<Vec3> {
    let room = occ.clearance(p);
    if room >= PROBE_CLEARANCE {
        return Some(p);
    }
    let h = occ.cell;
    let d = |v: Vec3| occ.clearance(p + v * h) - occ.clearance(p - v * h);
    let gradient = Vec3::new(d(Vec3::X), d(Vec3::Y), d(Vec3::Z)).normalize_or_zero();
    let moved = p + gradient * (PROBE_CLEARANCE - room).min(most);
    (!occ.is_solid(moved)).then_some(moved)
}
