//! src/components/rigidbody.rs — Rigidbody component
//!
//! mass/velocity/gravity/kinematic. Unity: Rigidbody. Moved verbatim from the
//! legacy `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Per-tick collision-testing strategy for a body (Unity's
/// `Rigidbody.collisionDetectionMode`, reduced to two modes). `Discrete` only
/// tests overlap at the tick's final pose — cheap, but a small/fast body can
/// tunnel clean through thin geometry between ticks. `Continuous` sweeps the
/// body's motion from its previous pose to its new pose within the tick (CCD),
/// at extra solver cost, so it never skips past thin/static colliders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionDetection {
    #[default]
    Discrete,
    Continuous,
}

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
    /// Discrete vs. continuous (CCD) collision testing (#321). Defaults to
    /// `Discrete` so pre-existing scene documents load unchanged.
    #[serde(default)]
    pub collision_detection: CollisionDetection,
}
