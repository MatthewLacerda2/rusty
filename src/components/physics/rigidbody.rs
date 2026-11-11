//! src/components/physics/rigidbody.rs — Rigidbody component
//!
//! mass/velocity/gravity/kinematic/drag. Unity: Rigidbody. Moved verbatim from the
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
    /// Linear damping, ≥ 0 (Unity: `Rigidbody.drag`, default `0`): rapier scales
    /// the body's linear velocity by `1 / (1 + dt·drag)` each tick, so a falling
    /// body reaches a terminal speed and thrown debris slows in the air.
    #[serde(default)]
    pub drag: f32,
    /// Angular damping, ≥ 0 (Unity: `Rigidbody.angularDrag`, default `0.05`): the
    /// same decay on the spin, so a barrel rolling on its side comes to rest.
    #[serde(default = "default_angular_drag")]
    pub angular_drag: f32,
}

/// Unity's `Rigidbody.angularDrag` default.
pub const DEFAULT_ANGULAR_DRAG: f32 = 0.05;

fn default_angular_drag() -> f32 {
    DEFAULT_ANGULAR_DRAG
}

impl crate::components::FirstClass for RigidBodyComponent {
    const NAME: &'static str = "RigidBody";
    const INVENTORY: &'static str = "Rigidbody";
}
