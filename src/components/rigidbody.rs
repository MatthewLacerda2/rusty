//! src/components/rigidbody.rs — Rigidbody component
//!
//! mass/velocity/gravity/kinematic. Unity: Rigidbody. Moved verbatim from the
//! legacy `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBodyComponent {
    pub active: bool,
    pub is_kinematic: bool,
    pub mass: f32,
    pub velocity: Vec3,
    /// Angular velocity in radians/sec per axis (Unity: `Rigidbody.angularVelocity`).
    /// rapier integrates a dynamic body's rotation from this each tick; a
    /// kinematic body's rotation instead comes from the corrected next pose, so
    /// this field is inert there (#319).
    #[serde(default)]
    pub angular_velocity: Vec3,
    pub use_gravity: bool,
}
