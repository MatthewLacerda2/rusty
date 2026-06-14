//! src/components/nav_agent.rs — NavMeshAgent component
//!
//! first-class agent that interfaces with the engine-baked navmesh. Unity:
//! NavMeshAgent. Moved verbatim from the legacy `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavMeshAgentComponent {
    pub active: bool,
    pub radius: f32,
    pub target: Vec3,
    pub speed: f32,
    pub acceleration: f32,
    pub stopping_distance: f32,
    pub velocity: Vec3,
}
