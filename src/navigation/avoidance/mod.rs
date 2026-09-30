//! Local avoidance between NavMesh agents (#463): ORCA — Optimal Reciprocal
//! Collision Avoidance (J. van den Berg, S. J. Guy, M. Lin, D. Manocha,
//! "Reciprocal n-Body Collision Avoidance", ISRR 2009 / Springer STAR 70, 2011),
//! following the reference RVO2 library's construction.
//!
//! Each steering agent turns every nearby agent into a half-plane of velocities
//! that keep the pair collision-free for [`TIME_HORIZON`] seconds, then picks the
//! velocity closest to its preferred one inside all of them (`lp`). The steering
//! tick feeds the preferred velocities in and integrates what comes out, so the
//! existing walkable-cell slide still has the last word on where an agent ends up.
//!
//! **Priority** follows Unity's `avoidancePriority` (lower = more important): an
//! agent ignores neighbours with a higher number, splits the dodge 50/50 with
//! equals (ORCA's reciprocity), and takes the whole dodge against a more important
//! one — or against any neighbour that won't dodge back (avoidance off, stopped).
//!
//! **Determinism.** Agents arrive in the ECS's stable insertion order; the
//! neighbour grid is only ever *looked up*, never iterated, and each neighbour list
//! is sorted by (distance, index) — so the half-plane order, and with it the LP
//! result, is a pure function of the inputs.
//!
//! **Obstacle hook.** `NavMeshObstacle` (#456) is not built yet. A moving,
//! non-carving obstacle joins as an [`AvoidanceAgent`] with `solves: false`
//! (every agent then takes full responsibility for it, like a Unity obstacle) —
//! no solver change needed. RVO2's static-obstacle lines would go in front of the
//! agent lines, with `lp3`'s "obstacle lines are hard" prefix.

mod lp;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use glam::Vec2;

use lp::Line;

/// How far ahead (seconds) ORCA guarantees agents stay apart.
pub const TIME_HORIZON: f32 = 2.0;
/// Radius (world units, XZ) within which other agents are considered.
pub const NEIGHBOR_DISTANCE: f32 = 10.0;
/// At most this many closest neighbours shape one agent's velocity.
pub const MAX_NEIGHBORS: usize = 10;

/// One agent's avoidance input, on the XZ plane (`Vec2(x, z)`).
#[derive(Clone, Copy, Debug)]
pub struct AvoidanceAgent {
    pub position: Vec2,
    /// The velocity it moved with last frame — what its neighbours predict.
    pub velocity: Vec2,
    /// The velocity it wants this frame (toward its next waypoint).
    pub preferred: Vec2,
    pub radius: f32,
    pub max_speed: f32,
    /// Unity's 0–99 `avoidancePriority`; lower is more important.
    pub priority: u8,
    /// Whether this agent runs the solver (and so dodges back reciprocally).
    pub solves: bool,
}

/// This agent's share of the dodge against `other`, or `None` to ignore it.
fn responsibility(me: &AvoidanceAgent, other: &AvoidanceAgent) -> Option<f32> {
    if !other.solves || me.priority > other.priority {
        Some(1.0)
    } else if me.priority < other.priority {
        None
    } else {
        Some(0.5)
    }
}

/// New velocity for every agent: the ORCA solution for each `solves` agent, the
/// unchanged preferred velocity for the rest.
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
                return me.preferred; // Alone: exactly the pre-avoidance steering.
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

/// Bucket agent indices by a grid of `NEIGHBOR_DISTANCE`-sized cells, so a query
/// only scans the 3×3 block around it. Buckets fill in index order.
fn build_grid(agents: &[AvoidanceAgent]) -> HashMap<(i32, i32), Vec<usize>> {
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, a) in agents.iter().enumerate() {
        grid.entry(cell_of(a.position)).or_default().push(i);
    }
    grid
}

/// The up-to-`MAX_NEIGHBORS` closest other agents within range, closest first
/// (ties by index).
fn neighbors(
    agents: &[AvoidanceAgent],
    grid: &HashMap<(i32, i32), Vec<usize>>,
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

/// The ORCA half-plane `me` must keep to against `other`, taking `share` of the
/// avoiding velocity change (RVO2 `Agent::computeNewVelocity`, agent part).
fn orca_line(me: &AvoidanceAgent, other: &AvoidanceAgent, share: f32, dt: f32) -> Option<Line> {
    let rel_pos = other.position - me.position;
    let rel_vel = me.velocity - other.velocity;
    let dist_sq = rel_pos.length_squared();
    let r = me.radius + other.radius;
    let r_sq = r * r;
    let (direction, u) = if dist_sq > r_sq {
        // No collision yet: project onto the truncated velocity-obstacle cone.
        let w = rel_vel - rel_pos / TIME_HORIZON;
        let w_len_sq = w.length_squared();
        let dot = w.dot(rel_pos);
        if dot < 0.0 && dot * dot > r_sq * w_len_sq {
            // Closest to the cut-off circle.
            let w_len = w_len_sq.sqrt();
            let unit_w = w / w_len;
            (
                Vec2::new(unit_w.y, -unit_w.x),
                (r / TIME_HORIZON - w_len) * unit_w,
            )
        } else {
            // Closest to a leg of the cone.
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
        // Already overlapping: resolve within one step.
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
