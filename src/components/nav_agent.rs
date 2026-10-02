//! src/components/nav_agent.rs — NavMeshAgent component
//!
//! first-class agent that interfaces with the engine-baked navmesh. Unity:
//! NavMeshAgent. Moved verbatim from the legacy `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Unity's default `avoidancePriority`: the middle of the 0–99 range.
pub const DEFAULT_AVOIDANCE_PRIORITY: u8 = 50;
/// The highest (least important) avoidance priority, as in Unity.
pub const MAX_AVOIDANCE_PRIORITY: u8 = 99;

/// Whether a computed path reaches its target (Unity's `NavMeshPathStatus`, #458).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavPathStatus {
    /// The path ends on the target.
    Complete,
    /// The target is unreachable; the path ends on the reachable point nearest to it.
    Partial,
    /// No path: the start or the target has no navmesh nearby (or none was planned).
    #[default]
    Invalid,
}

impl NavPathStatus {
    /// The lowercase name scripts see (`"complete"`, `"partial"`, `"invalid"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavMeshAgentComponent {
    pub active: bool,
    pub radius: f32,
    pub target: Vec3,
    pub speed: f32,
    pub acceleration: f32,
    pub stopping_distance: f32,
    pub velocity: Vec3,
    /// Local-avoidance importance, 0–99 (Unity's `avoidancePriority`): the
    /// **lower** the number, the more important the agent. An agent ignores
    /// neighbours with a higher number (they yield to it), splits the dodge
    /// with equals, and fully yields to lower numbers (#463).
    #[serde(default = "default_avoidance_priority")]
    pub avoidance_priority: u8,
    /// Whether this agent steers around other agents (Unity's
    /// `obstacleAvoidanceType != None`). A non-avoiding agent is still a
    /// neighbour the others steer around; it just never dodges itself.
    #[serde(default = "default_true")]
    pub avoidance_enabled: bool,

    // --- Cached pathfinding state (#126) ---
    //
    // Runtime-only: the navmesh re-derives these every Play, so they must never
    // bloat scene files nor break pre-existing saves. `#[serde(skip)]` omits them
    // on save and defaults them on load. `tick_nav_agents` plans an A* path once
    // and walks `path_cursor` along it, re-planning only when this cache is
    // invalidated (target moved, navmesh rebaked, next waypoint blocked, path
    // exhausted, or staleness) — instead of a full A* search every frame.
    /// World-space waypoints the agent steers through (empty = no valid path).
    #[serde(skip)]
    pub cached_path: Vec<Vec3>,
    /// Index of the waypoint in `cached_path` the agent is currently steering to.
    #[serde(skip)]
    pub path_cursor: usize,
    /// The target the cached path was planned for; a meaningful move re-plans.
    #[serde(skip)]
    pub planned_target: Vec3,
    /// Navmesh bake generation the path was planned against; a rebake re-plans.
    #[serde(skip)]
    pub path_generation: u64,
    /// Frames since the last re-plan; a bounded counter forces periodic refresh
    /// (frame-count based, never wall-clock, so the sim stays deterministic).
    #[serde(skip)]
    pub frames_since_replan: u32,
    /// Whether the cached path reaches the target (#458). A partial path ends on the
    /// reachable point nearest the target, and the agent stops there.
    #[serde(skip)]
    pub path_status: NavPathStatus,
}

impl NavMeshAgentComponent {
    /// Where the agent is actually headed: the target, or the end of a partial path
    /// (Unity stops an agent there rather than pressing into the wall).
    pub fn destination(&self) -> Vec3 {
        match (self.path_status, self.cached_path.last()) {
            (NavPathStatus::Partial, Some(end)) => *end,
            _ => self.target,
        }
    }
}

fn default_avoidance_priority() -> u8 {
    DEFAULT_AVOIDANCE_PRIORITY
}

fn default_true() -> bool {
    true
}

impl Default for NavMeshAgentComponent {
    fn default() -> Self {
        Self {
            active: false,
            radius: 0.0,
            target: Vec3::ZERO,
            speed: 0.0,
            acceleration: 0.0,
            stopping_distance: 0.0,
            velocity: Vec3::ZERO,
            avoidance_priority: DEFAULT_AVOIDANCE_PRIORITY,
            avoidance_enabled: true,
            cached_path: Vec::new(),
            path_cursor: 0,
            planned_target: Vec3::ZERO,
            path_generation: 0,
            frames_since_replan: 0,
            path_status: NavPathStatus::Invalid,
        }
    }
}
