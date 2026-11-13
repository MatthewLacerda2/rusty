//! crates/components/src/rendering/camera/ — Camera component (this `mod.rs`): fov /
//! near / far / motion blur, plus the camera-stack knobs (`render_order`,
//! `clear_flags`). Unity: Camera. The field of view's display axis and its
//! conversions sit in `fov_axis` (#2038), the render-texture target in `target`.

use serde::{Deserialize, Serialize};

mod fov_axis;
mod target;

pub use fov_axis::{horizontal_to_vertical_fov, vertical_to_horizontal_fov, FovAxis};
pub use target::{is_render_texture, RenderTarget, RENDER_TEXTURE_PREFIX};

use rusty_core::settings::quality::EffectQuality;

/// A fresh camera renders every layer (Unity's default Culling Mask = Everything).
/// A scene file that leaves out `culling_mask` loads with this all-on value.
fn default_culling_mask() -> u32 {
    u32::MAX
}

rusty_core::named_enum! {
    /// How a stacked camera initializes the framebuffer before it draws (Unity's
    /// `Camera.clearFlags`). The renderer sorts active cameras by
    /// [`CameraComponent::render_order`] and applies each pass's clear behavior in
    /// turn, so later cameras composite over earlier ones (#93). Scripts also accept
    /// Unity's `CameraClearFlags` spellings, `"Color"` and `"Depth"`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ClearFlags {
        /// Clear color to the skybox/backdrop and clear depth — a world/base camera.
        #[default]
        Skybox = "Skybox",
        /// Clear color to a solid backdrop and clear depth.
        SolidColor = "SolidColor" | "Color",
        /// Clear depth only, PRESERVING the color already drawn — an FPS viewmodel or
        /// UI/overlay camera that draws on top of the world without wall-clipping.
        DepthOnly = "DepthOnly" | "Depth",
    }
}

/// A camera whose file names no `clear_flags` loads as a world/base camera.
fn default_clear_flags() -> ClearFlags {
    ClearFlags::Skybox
}

/// A camera whose file names no `fxaa_active` loads with anti-aliasing **on** — a
/// default-on effect, so a scene gains it rather than having to opt in. FXAA costs a fraction of a millisecond, so the off-switch is for
/// the rare shot that wants raw edges, not a posture scenes must opt out of.
fn default_fxaa_active() -> bool {
    true
}

rusty_core::named_enum! {
    /// TAA's quality (#1847, URP's `TemporalAAQuality`): each level runs URP's
    /// `DoTemporalAA` steps for it (`render::postfx::effects::taa::params::levels`),
    /// from `VeryLow`'s cross box and undilated motion to `VeryHigh`'s variance
    /// clipping and re-filtered pixel; `High` is URP's default. rusty's reveal
    /// (#1846) and reactive (#1845) rules run at every level.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraComponent {
    pub active: bool,
    /// The **vertical** field of view, in degrees (Unity's `fieldOfView`).
    pub fov: f32,
    /// Which axis the Inspector shows and edits `fov` along (#2038, Unity's
    /// *FOV Axis*); display only, `fov` stays vertical.
    #[serde(default)]
    pub fov_axis: FovAxis,
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
    /// Which tap count runs (#1522): a level of the active quality preset, or
    /// `motion_blur_samples` when `Custom`.
    #[serde(default)]
    pub motion_blur_quality: EffectQuality,
    /// `Custom`: the taps per pixel.
    pub motion_blur_samples: u32,
    /// Motion blur's *Intensity* (#1436, Unity's): the shutter fraction, how much of
    /// a frame's motion one frame smears. `0.5` is a 180° shutter.
    #[serde(default = "default_motion_blur_intensity")]
    pub motion_blur_intensity: f32,
    /// Motion blur's *Clamp* (#1436, Unity's): the longest streak, as a fraction of
    /// the view's height, so a fast flick never smears the whole screen.
    #[serde(default = "default_motion_blur_max_length")]
    pub motion_blur_max_length: f32,
    /// Run the FXAA pass at the end of the post-FX chain (#360). Defaults to `true`
    /// (see `default_fxaa_active`) — the same default-on posture as motion blur,
    /// because both are cheap whole-image effects a game wants unless it says
    /// otherwise. It has no quality knob: it is exactly the anti-aliasing an iGPU
    /// floor wants (MSAA being the expensive alternative).
    #[serde(default = "default_fxaa_active")]
    pub fxaa_active: bool,
    /// Run temporal anti-aliasing (#1702): jitter the projection a sub-pixel each
    /// frame and resolve against the view's history through its motion vectors.
    /// Off by default, as URP's camera is: it costs a history target and can ghost.
    /// With it on, TAA is the camera's anti-aliasing and the FXAA pass skips
    /// (Unity's camera has one anti-aliasing mode).
    #[serde(default)]
    pub taa_active: bool,
    /// TAA's quality (#1847, URP's): how much of the resolve runs. Default `High`.
    #[serde(default)]
    pub taa_quality: TaaQuality,
    /// URP's *Base Blend Factor* (#1847): the history's share for a still pixel; the
    /// resolve's frame influence is `1 - base`, so `0.9` is URP's 0.1 influence.
    #[serde(default = "default_taa_base_blend_factor")]
    pub taa_base_blend_factor: f32,
    /// URP's *Variance Clamp Scale* (#1847): the history box is the mean ± this many
    /// deviations. Lower reduces ghosting, higher reduces flicker.
    #[serde(default = "default_taa_variance_clamp_scale")]
    pub taa_variance_clamp_scale: f32,
    /// URP's *Jitter Scale* (#1847): scales the sub-pixel jitter; `1` is URP's.
    #[serde(default = "default_taa_jitter_scale")]
    pub taa_jitter_scale: f32,
    /// URP's *Mip Bias* (#1847): a `-1`..`0` texture mip bias, added to the negative
    /// bias TAAU already applies while upscaling.
    #[serde(default)]
    pub taa_mip_bias: f32,
    /// URP's TAA *Contrast Adaptive Sharpening* (#1847): AMD's RCAS on the tonemapped image,
    /// `0`..`1`; `0` (default) runs no sharpen, leaving the frame untouched.
    #[serde(default)]
    pub taa_contrast_adaptive_sharpening: f32,
    /// Perspective (the default) or orthographic — a top-down minimap (#430).
    #[serde(default)]
    pub projection: Projection,
    /// Occlusion Culling (#1714, Unity's `Camera.useOcclusionCulling`): skip the
    /// static meshes the scene's baked PVS says this camera's cell cannot see, and,
    /// under a quality preset with `gpu_occlusion_culling`, the opaque meshes its own
    /// depth hides (#1963, no bake needed). On by default, as Unity's camera is.
    #[serde(default = "default_use_occlusion_culling")]
    pub use_occlusion_culling: bool,
    /// Draw into a named render texture instead of the screen (#430): the camera
    /// leaves the screen stack and UI `Image`s / material maps show it as
    /// `"rt:<name>"`. `None` (the default) is an ordinary screen camera.
    #[serde(default)]
    pub target_texture: Option<RenderTarget>,
}

impl CameraComponent {
    /// Fewest motion-blur taps the renderer draws.
    pub const MIN_MOTION_BLUR_SAMPLES: u32 = 2;
    /// Most motion-blur taps the renderer draws; the default camera uses it.
    pub const MAX_MOTION_BLUR_SAMPLES: u32 = 32;
    /// The motion-blur intensity a camera starts with: a 180° shutter.
    pub const DEFAULT_MOTION_BLUR_INTENSITY: f32 = 0.5;
    /// The highest motion-blur intensity: the shutter open the whole frame.
    pub const MAX_MOTION_BLUR_INTENSITY: f32 = 1.0;
    /// The longest motion-blur streak a camera starts with, as a fraction of the
    /// view's height.
    pub const DEFAULT_MOTION_BLUR_MAX_LENGTH: f32 = 0.05;
    /// The highest motion-blur clamp (Unity's upper bound, 0.2).
    pub const MAX_MOTION_BLUR_MAX_LENGTH: f32 = 0.2;

    /// TAA's *Base Blend Factor* (URP's camera editor): default a 0.1 frame
    /// influence, within the slider's bounds.
    pub const DEFAULT_TAA_BASE_BLEND_FACTOR: f32 = 0.9;
    pub const MIN_TAA_BASE_BLEND_FACTOR: f32 = 0.6;
    pub const MAX_TAA_BASE_BLEND_FACTOR: f32 = 0.98;
    /// TAA's *Variance Clamp Scale* (URP's camera editor): default and bounds.
    pub const DEFAULT_TAA_VARIANCE_CLAMP_SCALE: f32 = 0.9;
    pub const MIN_TAA_VARIANCE_CLAMP_SCALE: f32 = 0.6;
    pub const MAX_TAA_VARIANCE_CLAMP_SCALE: f32 = 1.2;
    /// The most negative TAA *Mip Bias* (URP's lower bound; 0 its default).
    pub const MIN_TAA_MIP_BIAS: f32 = -1.0;
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

/// Default vertical field of view, in degrees.
pub const DEFAULT_FOV: f32 = 45.0;
/// Default near/far clip planes — the free-fly editor camera's clip range, and the
/// fallback when no scene camera is authoritative.
pub const DEFAULT_NEAR: f32 = 0.1;
pub const DEFAULT_FAR: f32 = 200.0;

impl Default for CameraComponent {
    /// A plain perspective screen camera: the scene defaults' lens, every layer,
    /// skybox clear, FXAA on, TAA and motion blur off.
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
            projection: Projection::Perspective,
            use_occlusion_culling: default_use_occlusion_culling(),
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
