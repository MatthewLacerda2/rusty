//! src/components/nav_obstacle.rs — NavMeshObstacle (#456).
//!
//! Unity's `NavMeshObstacle`: a box or an upright capsule that navigation agents
//! keep out of. A **carving** obstacle cuts its footprint out of the navmesh, so
//! paths route around it (a closed door, a parked car); the cut grows by the agent
//! radius like any other edge. With **carve only stationary** (on by default, as in
//! Unity) it carves only once it has stood still for `time_to_stationary` seconds,
//! where "still" means it moved less than `move_threshold` since it last carved; a
//! pushed crate stops cutting holes while it slides. A carving obstacle that moves
//! more than the threshold re-carves at its new pose.
//!
//! An obstacle that is **not** carving (carving off, or moving) is a moving
//! obstacle the agents' local avoidance steers around instead.
//!
//! The box `size` and `center` follow the Transform (rotation and scale). The
//! capsule stays upright: `height` scales by the Transform's `y`, `radius` by the
//! larger of `x` and `z`. The runtime fields are never saved.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// The obstacle's shape (Unity's `NavMeshObstacleShape`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObstacleShape {
    #[default]
    Box,
    Capsule,
}

impl ObstacleShape {
    pub const ALL: [Self; 2] = [Self::Box, Self::Capsule];

    /// The name the Lua API and inspector show (`"Box"` / `"Capsule"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Box => "Box",
            Self::Capsule => "Capsule",
        }
    }

    /// Parse the Lua/editor name back, case-insensitively; `None` when unknown.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str().eq_ignore_ascii_case(s))
    }
}

/// A box or capsule navigation agents keep out of. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NavMeshObstacleComponent {
    /// Unity's `enabled`: an inactive obstacle neither carves nor is avoided.
    pub active: bool,
    pub shape: ObstacleShape,
    /// The shape's centre, in the entity's local space.
    pub center: Vec3,
    /// The box's full size (local, before scale). Box only.
    pub size: Vec3,
    /// The capsule's radius (before scale). Capsule only.
    pub radius: f32,
    /// The capsule's full height (before scale). Capsule only.
    pub height: f32,
    /// Cut the footprint out of the navmesh (Unity's `carving`).
    pub carve: bool,
    /// Carve only after standing still for `time_to_stationary`.
    pub carve_only_stationary: bool,
    /// How far (metres) the obstacle must move to count as moving (Unity's
    /// `carvingMoveThreshold`).
    pub move_threshold: f32,
    /// Seconds still before a stationary-only obstacle carves (Unity's
    /// `carvingTimeToStationary`).
    pub time_to_stationary: f32,
    /// The world pose the obstacle carves at: where it last stood still. `None`
    /// until the play-mode tick first sees it.
    #[serde(skip)]
    pub carve_pose: Option<Mat4>,
    /// Seconds since it last moved past `move_threshold`.
    #[serde(skip)]
    pub stationary_time: f32,
    /// World velocity over the last tick (what local avoidance predicts).
    #[serde(skip)]
    pub velocity: Vec3,
    /// World position at the last tick, for `velocity`.
    #[serde(skip)]
    pub last_position: Option<Vec3>,
}

impl Default for NavMeshObstacleComponent {
    /// Unity's defaults: a 1 m box at the pivot, not carving; carving, once
    /// enabled, waits 0.5 s of standing still past a 0.1 m threshold.
    fn default() -> Self {
        Self {
            active: true,
            shape: ObstacleShape::Box,
            center: Vec3::ZERO,
            size: Vec3::ONE,
            radius: 0.5,
            height: 2.0,
            carve: false,
            carve_only_stationary: true,
            move_threshold: 0.1,
            time_to_stationary: 0.5,
            carve_pose: None,
            stationary_time: 0.0,
            velocity: Vec3::ZERO,
            last_position: None,
        }
    }
}

impl NavMeshObstacleComponent {
    /// Whether the obstacle cuts the navmesh right now: carving on, and stationary
    /// long enough when it carves only while stationary. An obstacle the play tick
    /// has not seen yet (edit mode) carves where it stands.
    pub fn is_carving(&self) -> bool {
        self.active
            && self.carve
            && (self.carve_pose.is_none()
                || !self.carve_only_stationary
                || self.stationary_time >= self.time_to_stationary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_names_round_trip_case_insensitively() {
        for k in ObstacleShape::ALL {
            assert_eq!(ObstacleShape::parse(k.as_str()), Some(k));
        }
        assert_eq!(
            ObstacleShape::parse("capsule"),
            Some(ObstacleShape::Capsule)
        );
        assert_eq!(ObstacleShape::parse("cone"), None);
    }

    #[test]
    fn carving_waits_for_stationary_once_ticked() {
        let mut o = NavMeshObstacleComponent::default();
        assert!(!o.is_carving(), "carving is off by default");
        o.carve = true;
        assert!(
            o.is_carving(),
            "unticked (edit mode) carves where it stands"
        );
        o.carve_pose = Some(Mat4::IDENTITY);
        assert!(!o.is_carving(), "a ticked obstacle waits to be stationary");
        o.stationary_time = 0.5;
        assert!(o.is_carving());
        o.active = false;
        assert!(!o.is_carving());
    }
}
