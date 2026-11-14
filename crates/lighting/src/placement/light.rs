//! Light probes from the occupancy grid (#1737), after Unity's Adaptive Probe
//! Volumes: denser near geometry, sparse in open air, only where actors are.
//!
//! * **Where.** The air over the floor plan's floors (every storey, `upper`
//!   included) from [`LOWEST`] to [`VERTICAL_HEADROOM`](super::VERTICAL_HEADROOM)
//!   up, never past a cell's ceiling, never through a solid between the floor and
//!   the probe, and never out of sight of the floor cell it came from (the
//!   lattice reaches a little past each cell, and a thin wall must not let it into
//!   the next room). So nothing is placed inside a wall, above a ceiling, behind a
//!   wall or over floor no actor reaches (the floor plan holds only reachable
//!   floor when a nav surface is baked).
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
//! Every candidate is judged first and the survivors merged by lattice key after,
//! the lowest floor's winning, so a valid point never loses its key to an invalid
//! one from the floor below.
//!
//! The result has no regular grid, so it is sampled by blending the nearest
//! probes (`ProbeVolume::sample`). Deterministic: lattice order, no RNG.

use glam::{IVec3, Vec2, Vec3};

use super::occupancy::Occupancy;
use super::VERTICAL_HEADROOM;
use rusty_scene::lighting::floor::FloorGrid;

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
    // Each candidate is judged before the lattice keys are merged, so a valid
    // point (a coarse one on a walkway) never loses its key to an invalid one
    // from a lower floor (a ground point inside the walkway's slab).
    let mut kept: Vec<(IVec3, f32, Vec3)> = (lattice(floor, fine).into_iter())
        .filter_map(|p| judge(occ, &p, fine, spacing, manual).map(|at| (p.key, p.floor, at)))
        .collect();
    // One probe per lattice key: the lowest floor's, in key order.
    kept.sort_by(|a, b| {
        ((a.0.z, a.0.y, a.0.x).cmp(&(b.0.z, b.0.y, b.0.x))).then(a.1.total_cmp(&b.1))
    });
    kept.dedup_by_key(|k| k.0);
    kept.into_iter().map(|(_, _, at)| at).collect()
}

/// Where candidate `p` places a probe, if anywhere: a coarse point always, a fine
/// one only near geometry; pushed out of geometry; never above a ceiling, behind a
/// wall from its own floor cell, or within half a step of a `manual` probe.
fn judge(occ: &Occupancy, p: &Point, fine: f32, spacing: f32, manual: &[Vec3]) -> Option<Vec3> {
    let coarse = p.key.x % 2 == 0 && p.key.z % 2 == 0 && p.level % 2 == 0;
    if !coarse && !near_geometry(occ, p.at, fine) {
        return None;
    }
    let at = offset(occ, p.at, fine * 0.5)?;
    // The lattice reaches past its cell: through a thin wall it would light the
    // next room or the outdoors from in here.
    let sight = at - p.from;
    if sight.length() > 1.0e-4 && occ.ray(p.from, sight.normalize(), sight.length()).is_some() {
        return None;
    }
    // A solid between the floor and the probe: it is above a ceiling.
    let down = at.y - p.floor - occ.cell * 1.5;
    if down > 0.0 && occ.ray(at, Vec3::NEG_Y, down).is_some() {
        return None;
    }
    let step = if coarse { spacing } else { fine };
    (!manual.iter().any(|m| m.distance(at) < step * 0.5)).then_some(at)
}

/// A candidate probe: its lattice key, where it is, the floor it stands over, its
/// level over that floor (0 at [`LOWEST`]), and the point at its height over the
/// middle of the floor cell it came from, which it must see.
struct Point {
    key: IVec3,
    at: Vec3,
    floor: f32,
    level: i32,
    from: Vec3,
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
                    let height = y + LOWEST + level as f32 * fine;
                    points.push(Point {
                        key: IVec3::new(ix, base + level, iz),
                        at: Vec3::new(ix as f32 * fine, height, iz as f32 * fine),
                        floor: y,
                        level,
                        from: Vec3::new(at.x, height, at.y),
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
