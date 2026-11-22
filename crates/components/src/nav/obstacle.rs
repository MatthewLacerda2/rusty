use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ObstacleShape {
        #[default]
        Box = "Box",
        Capsule = "Capsule",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NavMeshObstacleComponent {
    pub active: bool,
    pub shape: ObstacleShape,
    pub center: Vec3,
    pub size: Vec3,
    pub radius: f32,
    pub height: f32,
    pub carve: bool,
    pub carve_only_stationary: bool,
    pub move_threshold: f32,
    pub time_to_stationary: f32,
    /// Where it last stood still; `None` until the play tick first sees it.
    #[serde(skip)]
    pub carve_pose: Option<Mat4>,
    #[serde(skip)]
    pub stationary_time: f32,
    /// World velocity over the last tick, which local avoidance predicts with.
    #[serde(skip)]
    pub velocity: Vec3,
    #[serde(skip)]
    pub last_position: Option<Vec3>,
}

impl Default for NavMeshObstacleComponent {
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
    pub fn is_carving(&self) -> bool {
        self.active
            && self.carve
            && (self.carve_pose.is_none()
                || !self.carve_only_stationary
                || self.stationary_time >= self.time_to_stationary)
    }
}

impl crate::FirstClass for NavMeshObstacleComponent {
    const NAME: &'static str = "NavMeshObstacle";
    const ALIASES: &'static [&'static str] = &["navobstacle"];
}

#[cfg(test)]
mod tests {
    use super::*;

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
