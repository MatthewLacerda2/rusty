//! src/components/nav_agent.rs — NavMeshAgent component
//!
//! first-class agent that interfaces with the engine-baked navmesh. Unity:
//! NavMeshAgent. Moved verbatim from the legacy `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NavMeshAgentComponent {
    pub active: bool,
    pub radius: f32,
    pub target: Vec3,
    pub speed: f32,
    pub acceleration: f32,
    pub stopping_distance: f32,
    pub velocity: Vec3,

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
}
