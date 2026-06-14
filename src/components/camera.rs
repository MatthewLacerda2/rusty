//! src/components/camera.rs — Camera component
//!
//! fov/near/far/motion-blur. Unity: Camera. Moved verbatim from the legacy
//! `core/scene.rs`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraComponent {
    pub active: bool,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub motion_blur_active: bool,
    pub motion_blur_samples: u32,
}
