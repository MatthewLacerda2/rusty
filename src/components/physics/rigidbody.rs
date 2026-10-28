//! src/components/physics/rigidbody.rs — Rigidbody component
//!
//! mass/velocity/gravity/kinematic. Unity: Rigidbody. Moved verbatim from the
//! former `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// How a rigid body's contacts are resolved each tick (Unity:
    /// `Rigidbody.collisionDetectionMode`, reduced to the two modes a game needs).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum CollisionDetection {
        /// Overlap is tested at the tick's poses; a fast dynamic body is still swept
        /// against *static* geometry (rapier does that for every fast body). Cheap;
        /// correct for slow/large bodies. The default for every body class.
        #[default]
        Discrete = "Discrete",
        /// The sweep extends to *moving* (kinematic and dynamic) bodies: the motion
        /// is swept from its previous pose to its new pose within the tick and
        /// stopped at the time of impact — rapier's full CCD. Prevents a fast, small
        /// body from tunnelling through a moving one between two fixed ticks (a
        /// 100 m/s bullet moves ~1.6 m per 60 Hz tick).
        Continuous = "Continuous",
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
    /// inject an initial one.
    #[serde(default)]
    pub angular_velocity: Vec3,
    pub use_gravity: bool,
    /// Discrete (default) or Continuous (CCD). `#[serde(default)]` → Discrete so
    /// every existing scene document loads unchanged.
    #[serde(default)]
    pub collision_detection: CollisionDetection,
}
