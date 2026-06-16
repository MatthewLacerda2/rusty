//! src/components/camera.rs — Camera component
//!
//! fov/near/far/motion-blur. Unity: Camera. Moved verbatim from the legacy
//! `core/scene.rs`.

use serde::{Deserialize, Serialize};

/// A fresh camera renders every layer (Unity's default Culling Mask = Everything).
/// Pre-#92 scenes have no `culling_mask`, so they load with this all-on value.
fn default_culling_mask() -> u32 {
    u32::MAX
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraComponent {
    pub active: bool,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    /// Layer membership bitmask (one bit per `LayerRegistry` slot): the camera
    /// renders an entity only when `culling_mask & (1 << entity.layer)` is set.
    #[serde(default = "default_culling_mask")]
    pub culling_mask: u32,
    pub motion_blur_active: bool,
    pub motion_blur_samples: u32,
}
