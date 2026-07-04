//! src/components/rigidbody.rs — Rigidbody component
//!
//! mass/velocity/gravity/kinematic. Unity: Rigidbody. Moved verbatim from the
//! legacy `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// How a rigid body's contacts are resolved each tick (Unity:
/// `Rigidbody.collisionDetectionMode`, reduced to the two modes a game needs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionDetection {
    /// Overlap is tested only at the tick's final pose. Cheap; correct for
    /// slow/large bodies. The default for every body class.
    #[default]
    Discrete,
    /// The body's motion is swept from its previous pose to its new pose within
    /// the tick and stopped at the time of impact — rapier's CCD. Prevents a
    /// fast, small body from tunnelling through thin geometry between two fixed
    /// ticks (a 100 m/s bullet moves ~1.6 m per 60 Hz tick).
    Continuous,
}

impl CollisionDetection {
    /// The Unity-style name the Lua API and inspector show (`"Discrete"` /
    /// `"Continuous"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discrete => "Discrete",
            Self::Continuous => "Continuous",
        }
    }

    /// Parse the Lua/editor name back to the mode, `None` on an unknown string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Discrete" => Some(Self::Discrete),
            "Continuous" => Some(Self::Continuous),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBodyComponent {
    pub active: bool,
    pub is_kinematic: bool,
    pub mass: f32,
    pub velocity: Vec3,
    /// Angular velocity in radians/sec per axis (Unity: `Rigidbody.angularVelocity`).
    /// rapier integrates the body's rotation from this every tick; the engine
    /// writes it back so scripts can read the spin, and pushes it so scripts can
    /// inject an initial one. `#[serde(default)]` keeps pre-#319 scenes loading.
    #[serde(default)]
    pub angular_velocity: Vec3,
    pub use_gravity: bool,
    /// Discrete (default) or Continuous (CCD). `#[serde(default)]` → Discrete so
    /// every existing scene document loads unchanged.
    #[serde(default)]
    pub collision_detection: CollisionDetection,
}
