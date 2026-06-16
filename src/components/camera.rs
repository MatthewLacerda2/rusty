//! src/components/camera.rs — Camera component
//!
//! fov/near/far/motion-blur, plus the camera-stack knobs (`render_order`,
//! `clear_flags`). Unity: Camera. Moved verbatim from the legacy `core/scene.rs`.

use serde::{Deserialize, Serialize};

/// A fresh camera renders every layer (Unity's default Culling Mask = Everything).
/// Pre-#92 scenes have no `culling_mask`, so they load with this all-on value.
fn default_culling_mask() -> u32 {
    u32::MAX
}

/// How a stacked camera initializes the framebuffer before it draws (Unity's
/// `Camera.clearFlags`). The renderer sorts active cameras by
/// [`CameraComponent::render_order`] and applies each pass's clear behavior in turn,
/// so later cameras composite over earlier ones (#93).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearFlags {
    /// Clear color to the skybox/backdrop and clear depth — a world/base camera.
    #[default]
    Skybox,
    /// Clear color to a solid backdrop and clear depth.
    SolidColor,
    /// Clear depth only, PRESERVING the color already drawn — an FPS viewmodel or
    /// UI/overlay camera that draws on top of the world without wall-clipping.
    DepthOnly,
}

/// Pre-#93 scenes have no `clear_flags`; they load as a world/base camera.
fn default_clear_flags() -> ClearFlags {
    ClearFlags::Skybox
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
    /// Stacking order (Unity's "Depth"): the renderer draws active cameras from
    /// lowest to highest `render_order`, so a higher value composites on top.
    #[serde(default)]
    pub render_order: i32,
    /// What this camera clears before drawing (see [`ClearFlags`]).
    #[serde(default = "default_clear_flags")]
    pub clear_flags: ClearFlags,
    pub motion_blur_active: bool,
    pub motion_blur_samples: u32,
}
