use glam::Vec3;

use super::super::{path_length, NavigationGraph};
use super::REPLAN_TARGET_EPSILON_SQ;
use rusty_scene::NavMeshAgentComponent;

/// World units from the requested point within which a `Warp` snaps onto the navmesh.
pub const WARP_SNAP_DISTANCE: f32 = 1.0;

pub fn remaining_corners(agent: &NavMeshAgentComponent, position: Vec3) -> Vec<Vec3> {
    if agent.cached_path.is_empty() {
        return Vec::new();
    }
    let rest = agent.cached_path.iter().skip(agent.path_cursor).copied();
    std::iter::once(agent.feet(position)).chain(rest).collect()
}

/// A target above the floor is reached on the floor under it. Until a path is planned for
/// the target, the target itself is the measure.
pub fn is_at_target(agent: &NavMeshAgentComponent, position: Vec3) -> bool {
    let drift = (agent.target - agent.planned_target).length_squared();
    let end = match agent.cached_path.last() {
        Some(end) if drift <= REPLAN_TARGET_EPSILON_SQ => *end,
        _ => agent.target,
    };
    agent.feet(position).distance(end) <= agent.stopping_distance
}

/// Infinity when it has no path, as Unity reports an unknown remaining distance.
pub fn remaining_distance(agent: &NavMeshAgentComponent, position: Vec3) -> f32 {
    if agent.cached_path.is_empty() {
        return f32::INFINITY;
    }
    path_length(&remaining_corners(agent, position))
}

fn drop_path(agent: &mut NavMeshAgentComponent) {
    agent.path_request = 0;
    agent.path_planned = false;
    agent.cached_path.clear();
    agent.path_links.clear();
    agent.path_cursor = 0;
}

pub fn reset_path(agent: &mut NavMeshAgentComponent, position: Vec3) {
    agent.target = position;
    drop_path(agent);
}

impl NavigationGraph {
    /// Plans one new path from the new position on the next tick, rather than sliding there.
    pub fn warp_agent(&self, agent: &mut NavMeshAgentComponent, point: Vec3) -> Option<Vec3> {
        let landed = self.sample_position(point, WARP_SNAP_DISTANCE)?;
        agent.velocity = Vec3::ZERO;
        agent.off_mesh_link = None;
        drop_path(agent);
        Some(agent.body(landed))
    }
}
