//! src/components/light.rs — Light component
//!
//! ambient/directional/point/spot. Unity: Light. Moved verbatim from the legacy
//! `core/scene.rs`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LightType {
    Ambient,
    Directional,
    Point,
    Spotlight,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightComponent {
    pub light_type: LightType,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone: f32, // Degrees
    pub outer_cone: f32, // Degrees
}
