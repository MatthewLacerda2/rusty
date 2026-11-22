use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::OffMeshLinkData;

// Unity's defaults: mid-range priority (lower is more important), a yaw rate in degrees/s;
// the ramp in degrees/s² is rusty's own (0 to 120°/s in 1/6 s).
pub const DEFAULT_AVOIDANCE_PRIORITY: u8 = 50;
const DEFAULT_ANGULAR_SPEED: f32 = 120.0;
const DEFAULT_ANGULAR_ACCELERATION: f32 = 720.0;
pub const MAX_AVOIDANCE_PRIORITY: u8 = 99;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum NavPathStatus {
        Complete = "complete",
        Partial = "partial",
        #[default]
        Invalid = "invalid",
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
    #[serde(default = "default_avoidance_priority")]
    pub avoidance_priority: u8,
    #[serde(default = "default_true")]
    pub avoidance_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_traverse_off_mesh_link: bool,
    #[serde(default)]
    pub base_offset: f32,
    #[serde(default = "default_area_mask")]
    pub area_mask: u32,
    #[serde(default = "default_true")]
    pub update_rotation: bool,
    #[serde(default = "default_angular_speed")]
    pub angular_speed: f32,
    #[serde(default = "default_angular_acceleration")]
    pub angular_acceleration: f32,

    // Runtime-only path cache: re-derived each Play, re-planned only once invalidated.
    #[serde(skip)]
    pub cached_path: Vec<Vec3>,
    #[serde(skip)]
    pub path_cursor: usize,
    #[serde(skip)]
    pub planned_target: Vec3,
    #[serde(skip)]
    pub path_generation: u64,
    /// The queued request's ticket, 0 when none.
    #[serde(skip)]
    pub path_request: u32,
    /// Set, an empty `cached_path` means no path can be planned, not one still to ask for.
    #[serde(skip)]
    pub path_planned: bool,
    /// The feet position its answer holds only at (a search cut short, or a start off-mesh).
    #[serde(skip)]
    pub replan_from: Option<Vec3>,
    #[serde(skip)]
    pub path_status: NavPathStatus,
    /// `(i, link)`: the leg into waypoint `i` crosses `link`.
    #[serde(skip)]
    pub path_links: Vec<(usize, OffMeshLinkData)>,
    #[serde(skip)]
    pub off_mesh_link: Option<OffMeshLinkData>,
    /// Signed yaw rate in degrees/second, positive counter-clockwise seen from above.
    #[serde(skip)]
    pub angular_velocity: f32,
}

impl NavMeshAgentComponent {
    pub fn set_area_mask(&mut self, mask: u32) {
        if self.area_mask != mask {
            self.area_mask = mask;
            self.cached_path.clear();
            self.path_planned = false;
            self.path_request = 0;
        }
    }

    pub fn feet(&self, position: Vec3) -> Vec3 {
        position - Vec3::Y * self.base_offset
    }

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
            path_request: 0,
            path_planned: false,
            replan_from: None,
            path_status: NavPathStatus::Invalid,
            path_links: Vec::new(),
            off_mesh_link: None,
            angular_velocity: 0.0,
        }
    }
}

impl crate::FirstClass for NavMeshAgentComponent {
    const NAME: &'static str = "NavMeshAgent";
    const ALIASES: &'static [&'static str] = &["navagent"];
}
