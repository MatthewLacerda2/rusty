use serde::{Deserialize, Serialize};

mod fov_axis;
mod target;

pub use fov_axis::{horizontal_to_vertical_fov, vertical_to_horizontal_fov, FovAxis};
pub use target::{is_render_texture, RenderTarget, RENDER_TEXTURE_PREFIX};

use rusty_core::settings::quality::EffectQuality;

fn default_culling_mask() -> u32 {
    u32::MAX
}

rusty_core::named_enum! {
    /// Unity's `Camera.clearFlags`. Scripts also accept `"Color"` and `"Depth"`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ClearFlags {
        #[default]
        Skybox = "Skybox",
        SolidColor = "SolidColor" | "Color",
        /// Depth only, preserving the color already drawn: an FPS viewmodel or overlay camera.
        DepthOnly = "DepthOnly" | "Depth",
    }
}

fn default_clear_flags() -> ClearFlags {
    ClearFlags::Skybox
}

/// FXAA is default-on; the off-switch is for shots that want raw edges.
fn default_fxaa_active() -> bool {
    true
}

rusty_core::named_enum! {
    /// URP's `TemporalAAQuality`; `High` is URP's default.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TaaQuality {
        VeryLow = "VeryLow",
        Low = "Low",
        Medium = "Medium",
        #[default]
        High = "High",
        VeryHigh = "VeryHigh",
    }
}

fn default_taa_base_blend_factor() -> f32 {
    CameraComponent::DEFAULT_TAA_BASE_BLEND_FACTOR
}
fn default_taa_variance_clamp_scale() -> f32 {
    CameraComponent::DEFAULT_TAA_VARIANCE_CLAMP_SCALE
}
fn default_taa_jitter_scale() -> f32 {
    1.0
}
fn default_taa_anti_flicker() -> f32 {
    CameraComponent::DEFAULT_TAA_ANTI_FLICKER
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraComponent {
    pub active: bool,
    pub fov: f32,
    #[serde(default)]
    pub fov_axis: FovAxis,
    pub near: f32,
    pub far: f32,
    #[serde(default = "default_culling_mask")]
    pub culling_mask: u32,
    #[serde(default)]
    pub render_order: i32,
    #[serde(default = "default_clear_flags")]
    pub clear_flags: ClearFlags,
    pub motion_blur_active: bool,
    #[serde(default)]
    pub motion_blur_quality: EffectQuality,
    pub motion_blur_samples: u32,
    #[serde(default = "default_motion_blur_intensity")]
    pub motion_blur_intensity: f32,
    /// Motion blur *Clamp*: the longest streak, as a fraction of the view's height.
    #[serde(default = "default_motion_blur_max_length")]
    pub motion_blur_max_length: f32,
    #[serde(default = "default_fxaa_active")]
    pub fxaa_active: bool,
    /// With it on, the FXAA pass skips (Unity has one anti-aliasing mode).
    #[serde(default)]
    pub taa_active: bool,
    #[serde(default)]
    pub taa_quality: TaaQuality,
    #[serde(default = "default_taa_base_blend_factor")]
    pub taa_base_blend_factor: f32,
    /// URP's *Variance Clamp Scale*: the history box is the mean ± this many deviations.
    #[serde(default = "default_taa_variance_clamp_scale")]
    pub taa_variance_clamp_scale: f32,
    #[serde(default = "default_taa_jitter_scale")]
    pub taa_jitter_scale: f32,
    /// URP's *Mip Bias*: `-1`..`0`.
    #[serde(default)]
    pub taa_mip_bias: f32,
    #[serde(default)]
    pub taa_contrast_adaptive_sharpening: f32,
    /// HDRP's TAA *Anti-Flicker*, `0`..`1`; `0` is URP's resolve exactly (`docs/api/Graphics.md`).
    #[serde(default = "default_taa_anti_flicker")]
    pub taa_anti_flicker: f32,
    #[serde(default)]
    pub projection: Projection,
    /// Unity's `Camera.useOcclusionCulling`.
    #[serde(default = "default_use_occlusion_culling")]
    pub use_occlusion_culling: bool,
    #[serde(default)]
    pub target_texture: Option<RenderTarget>,
}

impl CameraComponent {
    pub const MIN_MOTION_BLUR_SAMPLES: u32 = 2;
    pub const MAX_MOTION_BLUR_SAMPLES: u32 = 32;
    pub const DEFAULT_MOTION_BLUR_INTENSITY: f32 = 0.5;
    pub const MAX_MOTION_BLUR_INTENSITY: f32 = 1.0;
    pub const DEFAULT_MOTION_BLUR_MAX_LENGTH: f32 = 0.05;
    pub const MAX_MOTION_BLUR_MAX_LENGTH: f32 = 0.2;

    pub const DEFAULT_TAA_BASE_BLEND_FACTOR: f32 = 0.9;
    pub const MIN_TAA_BASE_BLEND_FACTOR: f32 = 0.6;
    pub const MAX_TAA_BASE_BLEND_FACTOR: f32 = 0.98;
    pub const DEFAULT_TAA_VARIANCE_CLAMP_SCALE: f32 = 0.9;
    pub const MIN_TAA_VARIANCE_CLAMP_SCALE: f32 = 0.6;
    pub const MAX_TAA_VARIANCE_CLAMP_SCALE: f32 = 1.2;
    pub const MIN_TAA_MIP_BIAS: f32 = -1.0;
    pub const DEFAULT_TAA_ANTI_FLICKER: f32 = 0.5;
}

fn default_use_occlusion_culling() -> bool {
    true
}

fn default_motion_blur_intensity() -> f32 {
    CameraComponent::DEFAULT_MOTION_BLUR_INTENSITY
}

fn default_motion_blur_max_length() -> f32 {
    CameraComponent::DEFAULT_MOTION_BLUR_MAX_LENGTH
}

pub const DEFAULT_FOV: f32 = 45.0;
pub const DEFAULT_NEAR: f32 = 0.1;
pub const DEFAULT_FAR: f32 = 200.0;

impl Default for CameraComponent {
    fn default() -> Self {
        Self {
            active: true,
            fov: DEFAULT_FOV,
            fov_axis: FovAxis::Vertical,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
            culling_mask: default_culling_mask(),
            render_order: 0,
            clear_flags: default_clear_flags(),
            motion_blur_active: false,
            motion_blur_quality: EffectQuality::Medium,
            motion_blur_samples: 0,
            motion_blur_intensity: default_motion_blur_intensity(),
            motion_blur_max_length: default_motion_blur_max_length(),
            fxaa_active: default_fxaa_active(),
            taa_active: false,
            taa_quality: TaaQuality::default(),
            taa_base_blend_factor: default_taa_base_blend_factor(),
            taa_variance_clamp_scale: default_taa_variance_clamp_scale(),
            taa_jitter_scale: default_taa_jitter_scale(),
            taa_mip_bias: 0.0,
            taa_contrast_adaptive_sharpening: 0.0,
            taa_anti_flicker: Self::DEFAULT_TAA_ANTI_FLICKER,
            projection: Projection::Perspective,
            use_occlusion_culling: default_use_occlusion_culling(),
            target_texture: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Projection {
    #[default]
    Perspective,
    Orthographic {
        size: f32,
    },
}
