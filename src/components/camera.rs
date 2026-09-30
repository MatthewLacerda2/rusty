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

/// Pre-#360 scenes have no `fxaa_active`, so they load with anti-aliasing **on** —
/// which is the point of a default-on effect: existing scenes gain it rather than
/// having to opt in. FXAA costs a fraction of a millisecond, so the off-switch is for
/// the rare shot that wants raw edges, not a posture scenes must opt out of.
fn default_fxaa_active() -> bool {
    true
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
    /// Run the FXAA pass at the end of the post-FX chain (#360). Defaults to `true`
    /// (see `default_fxaa_active`) — the same default-on posture as motion blur,
    /// because both are cheap whole-image effects a game wants unless it says
    /// otherwise. Unlike motion blur this is *not* gated by the quality preset: FXAA
    /// runs on every tier including Low, where it is exactly the anti-aliasing an
    /// iGPU floor wants (MSAA being the expensive alternative).
    #[serde(default = "default_fxaa_active")]
    pub fxaa_active: bool,
    /// Perspective (the default) or orthographic — a top-down minimap (#430).
    #[serde(default)]
    pub projection: Projection,
    /// Draw into a named render texture instead of the screen (#430): the camera
    /// leaves the screen stack and UI `Image`s / material maps show it as
    /// `"rt:<name>"`. `None` (the default) is an ordinary screen camera.
    #[serde(default)]
    pub target_texture: Option<RenderTarget>,
}

impl Default for CameraComponent {
    /// A plain perspective screen camera: the scene defaults' lens, every layer,
    /// skybox clear, FXAA on, motion blur off.
    fn default() -> Self {
        Self {
            active: true,
            fov: 45.0,
            near: 0.1,
            far: 200.0,
            culling_mask: default_culling_mask(),
            render_order: 0,
            clear_flags: default_clear_flags(),
            motion_blur_active: false,
            motion_blur_samples: 0,
            fxaa_active: default_fxaa_active(),
            projection: Projection::Perspective,
            target_texture: None,
        }
    }
}

/// How a camera projects the world (Unity's `Camera.orthographic`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Projection {
    /// Perspective through the camera's vertical `fov`.
    #[default]
    Perspective,
    /// Parallel projection; `size` is half the view's height in world units
    /// (Unity's `orthographicSize`), the width following the aspect.
    Orthographic { size: f32 },
}

/// A camera's render-texture target (#430, Unity's `Camera.targetTexture`): the
/// texture it draws into and what that costs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderTarget {
    /// The texture's name; consumers reference it as `"rt:<name>"`.
    pub name: String,
    /// Resolution in pixels (each at least 1).
    pub width: u32,
    pub height: u32,
    /// Run the full post-FX chain (grading, bloom, FXAA, motion blur). Off, the
    /// camera is only tonemapped — cheaper, and right for a flat minimap.
    #[serde(default = "default_true")]
    pub post_fx: bool,
    /// Redraw every `n`th frame (1 = every frame); the texture holds its last
    /// picture in between — a security monitor at 1/4 rate is a quarter the cost.
    #[serde(default = "default_every")]
    pub update_every: u32,
}

impl RenderTarget {
    /// A target named `name` at `width` x `height`, post-FX on, every frame.
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self {
            name: name.to_string(),
            width: width.max(1),
            height: height.max(1),
            post_fx: true,
            update_every: 1,
        }
    }

    /// The path a UI `Image` or a material map uses to show this texture.
    pub fn path(&self) -> String {
        format!("{RENDER_TEXTURE_PREFIX}{}", self.name)
    }
}

/// The prefix that marks a texture path as a render texture's name (#430) — one
/// convention for every consumer (`Image.texture`, material maps).
pub const RENDER_TEXTURE_PREFIX: &str = "rt:";

fn default_true() -> bool {
    true
}

fn default_every() -> u32 {
    1
}
