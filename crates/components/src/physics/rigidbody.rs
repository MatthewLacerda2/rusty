use glam::Vec3;
use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum CollisionDetection {
        #[default]
        Discrete = "Discrete",
        Continuous = "Continuous",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBodyComponent {
    pub active: bool,
    pub is_kinematic: bool,
    pub mass: f32,
    pub velocity: Vec3,
    #[serde(default)]
    pub angular_velocity: Vec3,
    pub use_gravity: bool,
    #[serde(default)]
    pub collision_detection: CollisionDetection,
    #[serde(default)]
    pub drag: f32,
    #[serde(default = "default_angular_drag")]
    pub angular_drag: f32,
}

pub const DEFAULT_ANGULAR_DRAG: f32 = 0.05;

fn default_angular_drag() -> f32 {
    DEFAULT_ANGULAR_DRAG
}

impl crate::FirstClass for RigidBodyComponent {
    const NAME: &'static str = "RigidBody";
    const INVENTORY: &'static str = "Rigidbody";
}
