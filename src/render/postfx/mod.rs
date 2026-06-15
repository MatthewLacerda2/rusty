//! src/render/postfx/mod.rs — Post-process chain (color correction, bloom,
//! motion blur, real SSR) + quality presets.
//!
//! The scene no longer renders straight to the swapchain. It renders into an HDR
//! offscreen target; this module then runs a fullscreen post-FX chain
//! (`postfx.wgsl`) that finally reads the `VisualCorrectionComponent` /
//! `CameraComponent` knobs and writes the corrected image to the real target.
//!
//! `QualityPreset` gates which passes run and how big the bloom buffers are so an
//! integrated GPU can hold ~30fps (Low: no SSR/motion-blur, half-size bloom).

mod run;
mod setup;

pub use run::PostFxContext;

use glam::Mat4;

/// Scalability tier. Gates SSR + motion blur and the bloom buffer size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QualityPreset {
    /// iGPU floor: SSR off (cubemap only), motion blur off, quarter-res bloom.
    Low,
    /// Balanced: cubemap reflection, motion blur on, half-res bloom.
    #[default]
    Medium,
    /// Discrete GPU: real screen-space reflections, motion blur, half-res bloom.
    High,
}

impl QualityPreset {
    /// Does this tier run the real screen-space-reflection pass?
    pub fn screen_space_ssr(self) -> bool {
        matches!(self, QualityPreset::High)
    }

    /// Does this tier run the camera motion-blur pass?
    pub fn motion_blur(self) -> bool {
        !matches!(self, QualityPreset::Low)
    }

    /// Divisor applied to the bloom buffer resolution (smaller = cheaper).
    pub fn bloom_divisor(self) -> u32 {
        match self {
            QualityPreset::Low => 4,
            QualityPreset::Medium | QualityPreset::High => 2,
        }
    }
}

/// CPU-side mirror of the `PostParams` uniform in `postfx.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostParams {
    /// exposure (EV), contrast, saturation, gamma
    pub color: [f32; 4],
    /// bloom_intensity, bloom_threshold, tonemap index, bloom_enabled
    pub bloom: [f32; 4],
    /// blur direction, texel_x, texel_y, motion_blur_samples
    pub misc: [f32; 4],
    /// ssr_mode, motion_blur_active, motion_blur_scale, unused
    pub flags: [f32; 4],
    pub inv_view_proj: [f32; 16],
    pub prev_view_proj: [f32; 16],
    pub view_proj: [f32; 16],
    /// camera world position (xyz) + pad
    pub camera_pos: [f32; 4],
}

impl Default for PostParams {
    fn default() -> Self {
        let id = Mat4::IDENTITY.to_cols_array();
        Self {
            // Neutral pass-through: exposure 0, contrast 1, saturation 1, gamma 1,
            // no tonemap, no bloom/SSR/motion-blur. A scene with no *active*
            // visual-correction volume therefore looks exactly as it did before
            // the post-FX chain was introduced. The knobs only bite once a
            // VisualCorrectionComponent is active (see postfx_params.rs).
            color: [0.0, 1.0, 1.0, 1.0],
            bloom: [1.0, 0.8, 0.0, 0.0],
            misc: [0.0, 0.0, 0.0, 0.0],
            flags: [0.0, 0.0, 1.0, 0.0],
            inv_view_proj: id,
            prev_view_proj: id,
            view_proj: id,
            camera_pos: [0.0; 4],
        }
    }
}

/// A colour texture + its default view, kept together for the ping-pong buffers.
pub struct PfxTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

/// Owns every GPU resource for the post-process chain. One per `Renderer`.
pub struct PostFx {
    pub bright_pipeline: wgpu::RenderPipeline,
    pub blur_pipeline: wgpu::RenderPipeline,
    pub composite_pipeline: wgpu::RenderPipeline,
    pub io_layout: wgpu::BindGroupLayout,

    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    /// HDR scene colour the main pass draws into.
    pub scene_hdr: PfxTarget,
    /// Bloom ping-pong buffers (bright-pass / blur), at reduced resolution.
    pub bloom_a: PfxTarget,
    pub bloom_b: PfxTarget,
    pub bloom_size: (u32, u32),
    pub full_size: (u32, u32),

    /// Previous-frame view-projection, for camera motion blur velocity.
    pub prev_view_proj: Mat4,
}

/// HDR scene colour format — float so bright pixels survive for bloom/tonemap.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
