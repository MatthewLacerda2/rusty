//! ORCA, following RVO2's construction. The walkable-cell slide has the last word on where an agent
//! ends up.
//!
//! **Priority** follows Unity's `avoidancePriority` (lower = more important): an agent ignores
//! higher numbers, splits the dodge 50/50 with equals, and takes the whole dodge against a more
//! important one or any neighbour that won't dodge back.
//!
//! **Determinism.** The neighbour grid is only looked up, never iterated, and each neighbour list
//! is sorted by (distance, index).

mod lp;
#[cfg(test)]
mod orca_tests;
#[cfg(test)]
mod tests;

use rusty_core::collections::Map;

use glam::Vec2;

use lp::Line;

/// How far ahead (seconds) ORCA guarantees agents stay apart.
pub const TIME_HORIZON: f32 = 2.0;
/// Radius (world units, XZ) within which other agents are considered.
pub const NEIGHBOR_DISTANCE: f32 = 10.0;
pub const MAX_NEIGHBORS: usize = 10;

#[derive(Clone, Copy, Debug)]
pub struct AvoidanceAgent {
    pub position: Vec2,
    pub velocity: Vec2,
    pub preferred: Vec2,
    pub radius: f32,
    pub max_speed: f32,
    /// Unity's 0–99 `avoidancePriority`.
    pub priority: u8,
    pub solves: bool,
}

fn responsibility(me: &AvoidanceAgent, other: &AvoidanceAgent) -> Option<f32> {
    if !other.solves || me.priority > other.priority {
        Some(1.0)
    } else if me.priority < other.priority {
        None
    } else {
        Some(0.5)
    }
}

pub fn solve(agents: &[AvoidanceAgent], dt: f32) -> Vec<Vec2> {
    if dt <= 0.0 {
        return agents.iter().map(|a| a.preferred).collect();
    }
    let grid = build_grid(agents);
    agents
        .iter()
        .enumerate()
        .map(|(i, me)| {
            if !me.solves {
                return me.preferred;
            }
            let lines: Vec<Line> = neighbors(agents, &grid, i)
                .into_iter()
                .filter_map(|j| {
                    let share = responsibility(me, &agents[j])?;
                    orca_line(me, &agents[j], share, dt)
                })
                .collect();
            if lines.is_empty() {
                return me.preferred;
            }
            let (v, done) = lp::lp2(&lines, me.max_speed, me.preferred, false);
            if done < lines.len() {
                lp::lp3(&lines, done, me.max_speed, v)
            } else {
                v
            }
        })
        .collect()
}

fn cell_of(p: Vec2) -> (i32, i32) {
    let c = (p / NEIGHBOR_DISTANCE).floor();
    (c.x as i32, c.y as i32)
}

fn build_grid(agents: &[AvoidanceAgent]) -> Map<(i32, i32), Vec<usize>> {
    let mut grid: Map<(i32, i32), Vec<usize>> = Map::default();
    for (i, a) in agents.iter().enumerate() {
        grid.entry(cell_of(a.position)).or_default().push(i);
    }
    grid
}

fn neighbors(
    agents: &[AvoidanceAgent],
    grid: &Map<(i32, i32), Vec<usize>>,
    i: usize,
) -> Vec<usize> {
    let me = agents[i].position;
    let (cx, cz) = cell_of(me);
    let range_sq = NEIGHBOR_DISTANCE * NEIGHBOR_DISTANCE;
    let mut found: Vec<(f32, usize)> = Vec::new();
    for dx in -1..=1 {
        for dz in -1..=1 {
            let Some(bucket) = grid.get(&(cx + dx, cz + dz)) else {
                continue;
            };
            for &j in bucket {
                let d = (agents[j].position - me).length_squared();
                if j != i && d < range_sq {
                    found.push((d, j));
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    found
        .into_iter()
        .take(MAX_NEIGHBORS)
        .map(|(_, j)| j)
        .collect()
}

fn orca_line(me: &AvoidanceAgent, other: &AvoidanceAgent, share: f32, dt: f32) -> Option<Line> {
    let rel_pos = other.position - me.position;
    let rel_vel = me.velocity - other.velocity;
    let dist_sq = rel_pos.length_squared();
    let r = me.radius + other.radius;
    let r_sq = r * r;
    let (direction, u) = if dist_sq > r_sq {
        let w = rel_vel - rel_pos / TIME_HORIZON;
        let w_len_sq = w.length_squared();
        let dot = w.dot(rel_pos);
        if dot < 0.0 && dot * dot > r_sq * w_len_sq {
            let w_len = w_len_sq.sqrt();
            let unit_w = w / w_len;
            (
                Vec2::new(unit_w.y, -unit_w.x),
                (r / TIME_HORIZON - w_len) * unit_w,
            )
        } else {
            let leg = (dist_sq - r_sq).sqrt();
            let dir = if rel_pos.perp_dot(w) > 0.0 {
                Vec2::new(
                    rel_pos.x * leg - rel_pos.y * r,
                    rel_pos.x * r + rel_pos.y * leg,
                )
            } else {
                -Vec2::new(
                    rel_pos.x * leg + rel_pos.y * r,
                    -rel_pos.x * r + rel_pos.y * leg,
                )
            } / dist_sq;
            (dir, rel_vel.dot(dir) * dir - rel_vel)
        }
    } else {
        let w = rel_vel - rel_pos / dt;
        let unit_w = w.try_normalize()?;
        (
            Vec2::new(unit_w.y, -unit_w.x),
            (r / dt - w.length()) * unit_w,
        )
    };
    Some(Line {
        point: me.velocity + share * u,
        direction,
    })
}
