//! src/components/nav_agent.rs — NavMeshAgent component
//!
//! first-class agent that interfaces with the engine-baked navmesh. Unity:
//! NavMeshAgent. Moved verbatim from the legacy `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::OffMeshLinkData;

/// Unity's default `avoidancePriority`: the middle of the 0–99 range.
pub const DEFAULT_AVOIDANCE_PRIORITY: u8 = 50;
/// Unity's default `angularSpeed`, degrees/second.
pub const DEFAULT_ANGULAR_SPEED: f32 = 120.0;
/// The default yaw-rate ramp, degrees/second²: 0 → 120°/s in 1/6 s.
pub const DEFAULT_ANGULAR_ACCELERATION: f32 = 720.0;
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
    /// Top speed, m/s.
    pub speed: f32,
    /// How fast the agent speeds up and brakes, a constant rate in m/s² (Unity's
    /// meaning, #742): 0 → `speed` takes `speed / acceleration` seconds.
    pub acceleration: f32,
    /// How far from its path's end (feet to end, centre to centre) it comes to rest.
    pub stopping_distance: f32,
    /// The planar velocity the agent is moving at, m/s; zero once at rest.
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
    /// Whether the engine moves the agent across an off-mesh link itself (#462,
    /// Unity's `autoTraverseOffMeshLink`, on by default). Off, the agent stops on
    /// the link until a script calls `CompleteOffMeshLink`.
    #[serde(default = "default_true")]
    pub auto_traverse_off_mesh_link: bool,
    /// How far the entity's origin sits above the agent's feet (Unity's
    /// `baseOffset`, default 0): the agent stands its feet on the navmesh and its
    /// Transform this much higher, so a body centred on its origin rests on the
    /// floor instead of sinking half into it (#666).
    #[serde(default)]
    pub base_offset: f32,
    /// The navigation areas the agent may enter (#460, Unity's `areaMask`): bit `i`
    /// allows area `i`. Every area by default.
    #[serde(default = "default_area_mask")]
    pub area_mask: u32,
    /// Whether the agent turns itself to face where it steers (#744, Unity's
    /// `updateRotation`, on by default). Off, rotation is left to scripts.
    #[serde(default = "default_true")]
    pub update_rotation: bool,
    /// Top yaw rate while turning to face its heading, degrees/second (Unity's
    /// `angularSpeed`, default 120).
    #[serde(default = "default_angular_speed")]
    pub angular_speed: f32,
    /// How fast the yaw rate builds up and winds down, degrees/second² (#744; beyond
    /// Unity, whose agent turns at a constant `angularSpeed`). The agent eases into
    /// a turn and out of it onto its heading, without overshoot.
    #[serde(default = "default_angular_acceleration")]
    pub angular_acceleration: f32,

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
    /// The off-mesh links on the cached path (#462): `(i, link)` means the leg into
    /// waypoint `i` crosses `link`.
    #[serde(skip)]
    pub path_links: Vec<(usize, OffMeshLinkData)>,
    /// The link the agent is on now, if any (Unity's `isOnOffMeshLink`).
    #[serde(skip)]
    pub off_mesh_link: Option<OffMeshLinkData>,
    /// How far (world units) auto-traversal has carried the agent along that link.
    #[serde(skip)]
    pub link_progress: f32,
    /// The signed yaw rate the agent is turning at, degrees/second (+ is
    /// counter-clockwise seen from above); zero when it isn't turning.
    #[serde(skip)]
    pub angular_velocity: f32,
}

impl NavMeshAgentComponent {
    /// Where the agent's feet are when its Transform is at `position`.
    pub fn feet(&self, position: Vec3) -> Vec3 {
        position - Vec3::Y * self.base_offset
    }

    /// Where its Transform goes to stand its feet on `feet`.
    pub fn body(&self, feet: Vec3) -> Vec3 {
        feet + Vec3::Y * self.base_offset
    }
}

fn default_avoidance_priority() -> u8 {
    DEFAULT_AVOIDANCE_PRIORITY
}

fn default_area_mask() -> u32 {
    u32::MAX
}

fn default_angular_speed() -> f32 {
    DEFAULT_ANGULAR_SPEED
}

fn default_angular_acceleration() -> f32 {
    DEFAULT_ANGULAR_ACCELERATION
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
            auto_traverse_off_mesh_link: true,
            base_offset: 0.0,
            area_mask: u32::MAX,
            update_rotation: true,
            angular_speed: DEFAULT_ANGULAR_SPEED,
            angular_acceleration: DEFAULT_ANGULAR_ACCELERATION,
            cached_path: Vec::new(),
            path_cursor: 0,
            planned_target: Vec3::ZERO,
            path_generation: 0,
            frames_since_replan: 0,
            path_status: NavPathStatus::Invalid,
            path_links: Vec::new(),
            off_mesh_link: None,
            link_progress: 0.0,
            angular_velocity: 0.0,
        }
    }
}
