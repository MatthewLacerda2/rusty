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
    pub use_gravity: bool,
}
