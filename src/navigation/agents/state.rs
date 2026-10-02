//! An agent's path state as scripts read and steer it (#458): what is left of the
//! cached path, and the two ways a script interrupts it — `Warp` (teleport) and
//! `ResetPath` (stop). Unity's `NavMeshAgent.remainingDistance`, `path`, `Warp` and
//! `ResetPath`.

use glam::Vec3;

use super::super::{path_length, NavigationGraph};
use crate::scene::NavMeshAgentComponent;

/// How far from the requested point a `Warp` may land, in world units: the point is
/// snapped onto the navmesh within this distance, or the warp fails.
pub const WARP_SNAP_DISTANCE: f32 = 1.0;

/// The rest of the agent's path from where it stands: its position, then every
/// waypoint it has not reached yet. Empty when it has no path.
pub fn remaining_corners(agent: &NavMeshAgentComponent, position: Vec3) -> Vec<Vec3> {
    if agent.cached_path.is_empty() {
        return Vec::new();
    }
    let rest = agent.cached_path.iter().skip(agent.path_cursor).copied();
    std::iter::once(position).chain(rest).collect()
}

/// The distance left along the agent's path, or infinity when it has no path (Unity
/// reports an unknown remaining distance as infinity).
pub fn remaining_distance(agent: &NavMeshAgentComponent, position: Vec3) -> f32 {
    if agent.cached_path.is_empty() {
        return f32::INFINITY;
    }
    path_length(&remaining_corners(agent, position))
}

/// Forget the cached path, so the next tick plans afresh — once.
fn drop_path(agent: &mut NavMeshAgentComponent) {
    agent.cached_path.clear();
    agent.path_links.clear();
    agent.path_cursor = 0;
}

/// Stop following the path: the agent's destination becomes where it stands, so it
/// slows to a stop and plans nothing until it gets a new target.
pub fn reset_path(agent: &mut NavMeshAgentComponent, position: Vec3) {
    agent.target = position;
    drop_path(agent);
}

impl NavigationGraph {
    /// Teleport `agent` to the walkable point nearest `point` (within
    /// [`WARP_SNAP_DISTANCE`]) and return it, or `None` (nothing changes) when there
    /// is none. The agent keeps its target, stops dead (off any off-mesh link it was
    /// on), and plans one new path from
    /// the new position on its next tick, instead of sliding there or re-planning
    /// every frame.
    pub fn warp_agent(&self, agent: &mut NavMeshAgentComponent, point: Vec3) -> Option<Vec3> {
        let landed = self.sample_position(point, WARP_SNAP_DISTANCE)?;
        agent.velocity = Vec3::ZERO;
        agent.off_mesh_link = None;
        drop_path(agent);
        Some(landed)
    }
}
