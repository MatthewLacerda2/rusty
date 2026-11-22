use glam::{IVec3, Vec2, Vec3};

use super::occupancy::Occupancy;
use super::VERTICAL_HEADROOM;
use rusty_scene::lighting::floor::FloorGrid;

const LOWEST: f32 = 1.0;
pub const PROBE_CLEARANCE: f32 = 0.3;

pub fn plan_light_set(
    floor: &FloorGrid,
    occ: &Occupancy,
    spacing: f32,
    manual: &[Vec3],
) -> Vec<Vec3> {
    let fine = spacing.max(1.0) * 0.5;
    let mut kept: Vec<(IVec3, f32, Vec3)> = (lattice(floor, fine).into_iter())
        .filter_map(|p| judge(occ, &p, fine, spacing, manual).map(|at| (p.key, p.floor, at)))
        .collect();
    kept.sort_by(|a, b| {
        ((a.0.z, a.0.y, a.0.x).cmp(&(b.0.z, b.0.y, b.0.x))).then(a.1.total_cmp(&b.1))
    });
    kept.dedup_by_key(|k| k.0);
    kept.into_iter().map(|(_, _, at)| at).collect()
}

fn judge(occ: &Occupancy, p: &Point, fine: f32, spacing: f32, manual: &[Vec3]) -> Option<Vec3> {
    let coarse = p.key.x % 2 == 0 && p.key.z % 2 == 0 && p.level % 2 == 0;
    if !coarse && !near_geometry(occ, p.at, fine) {
        return None;
    }
    let at = offset(occ, p.at, fine * 0.5)?;
    let sight = at - p.from;
    if sight.length() > 1.0e-4 && occ.ray(p.from, sight.normalize(), sight.length()).is_some() {
        return None;
    }
    let down = at.y - p.floor - occ.cell * 1.5;
    if down > 0.0 && occ.ray(at, Vec3::NEG_Y, down).is_some() {
        return None;
    }
    let step = if coarse { spacing } else { fine };
    (!manual.iter().any(|m| m.distance(at) < step * 0.5)).then_some(at)
}

struct Point {
    key: IVec3,
    at: Vec3,
    floor: f32,
    level: i32,
    from: Vec3,
}

fn lattice(floor: &FloorGrid, fine: f32) -> Vec<Point> {
    let mut points = Vec::new();
    let floors = (floor.cells.iter().enumerate())
        .filter_map(|(i, c)| c.map(|c| (i, c.floor, c.ceiling)))
        .chain(floor.upper.iter().map(|&(i, y)| (i, y, f32::INFINITY)));
    let reach = Vec2::splat(floor.cell * 1.5 + floor.inset);
    for (cell, y, ceiling) in floors {
        let at = floor.centre(cell % floor.width, cell / floor.width);
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

fn near_geometry(occ: &Occupancy, p: Vec3, reach: f32) -> bool {
    [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
        .iter()
        .any(|&d| occ.ray(p, d, reach).is_some())
}

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
