//! src/render/postfx/mod.rs — Post-process chain (color correction, bloom,
//! per-object motion blur (`motion_blur`), real SSR) + quality presets.
//!
//! The scene no longer renders straight to the swapchain. It renders into an HDR
//! offscreen target; this module then runs a fullscreen post-FX chain
//! (`postfx.wgsl`) that finally reads the `PostProcessingComponent` /
//! `CameraComponent` knobs and writes the corrected image to the real target.
//!
//! [`QualityPreset`](crate::core::settings::quality::QualityPreset) (sim-side) gates which
//! passes run and how big the bloom buffers are so an integrated GPU can hold
//! ~30fps (Low: no SSR/motion-blur, half-size bloom).

mod custom;
pub(crate) mod exposure;
pub(crate) mod grade;
mod inputs;
pub(crate) mod motion_blur;
pub(crate) mod params;
mod run;
mod setup;

pub use inputs::{PostFxContext, PostPasses};

use glam::Mat4;

/// CPU-side mirror of the `PostParams` uniform in `postfx.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostParams {
    /// exposure (EV), contrast, saturation, gamma. The composite reads only the
    /// exposure: the rest reach it baked into the grading LUT (#1440).
    pub color: [f32; 4],
    /// bloom_intensity, bloom_threshold, tonemap index, bloom_enabled
    pub bloom: [f32; 4],
    /// blur direction, texel_x, texel_y, unused
    pub misc: [f32; 4],
    /// ssr_mode, unused, unused, auto exposure (1: the composite applies the
    /// view's adapted EV instead of `color.x`, #1444)
    pub flags: [f32; 4],
    pub inv_view_proj: [f32; 16],
    pub view_proj: [f32; 16],
    /// camera world position (xyz) + the sim's game time (w, #398) — authored
    /// postfx effects read it as `game_time()`
    pub camera_pos: [f32; 4],
}

impl Default for PostParams {
    fn default() -> Self {
        let id = Mat4::IDENTITY.to_cols_array();
        Self {
            // Neutral pass-through: exposure 0, contrast 1, saturation 1, gamma 1,
            // no tonemap, no bloom/SSR/motion-blur. A scene with no *active*
            // post-processing volume therefore looks exactly as it did before
            // the post-FX chain was introduced. The knobs only bite once a
            // PostProcessingComponent is active (see postfx_params.rs).
            color: [0.0, 1.0, 1.0, 1.0],
            bloom: [1.0, 0.8, 0.0, 0.0],
            misc: [0.0, 0.0, 0.0, 0.0],
            flags: [0.0; 4],
            inv_view_proj: id,
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
    /// Final anti-aliasing pass (#360), reading [`PostFx::ldr`] and writing the output.
    pub fxaa_pipeline: wgpu::RenderPipeline,
    /// Plain copy, landing the authored-effect chain's result in the output when
    /// FXAA is off (#397).
    pub copy_pipeline: wgpu::RenderPipeline,
    /// The grade + tonemap, baked into a 3D LUT the composite samples (#1440).
    pub grade: grade::GradeLut,
    /// The view's automatic-exposure meter and its adapted EV (#1444).
    pub exposure: exposure::ExposureMeter,
    /// Authored post-FX effects (#397): their module cache, second LDR target and
    /// previous-frame history.
    pub custom: custom::CustomChain,
    pub io_layout: wgpu::BindGroupLayout,

    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    /// HDR scene colour the main pass draws into.
    pub scene_hdr: PfxTarget,
    /// Bloom ping-pong buffers (bright-pass / blur), at reduced resolution.
    pub bloom_a: PfxTarget,
    pub bloom_b: PfxTarget,
    /// Full-resolution LDR image the composite writes when FXAA is on, so the FXAA
    /// pass has a finished, tonemapped, gamma-encoded frame to sample.
    ///
    /// This is the one target the chain gained for #360, and it is unavoidable rather
    /// than incidental: FXAA is defined on post-tonemap colour, so it cannot fold into
    /// the composite, and the existing ping-pong buffers are at the bloom divisor's
    /// reduced resolution and in HDR format. With FXAA off the composite writes
    /// straight to the output and this texture is simply never read — the off path
    /// stays a byte-exact pass-through.
    pub ldr: PfxTarget,
    pub bloom_size: (u32, u32),
    pub full_size: (u32, u32),
    /// The output format `ldr` and the composite/FXAA pipelines are built against,
    /// kept so `resize` can reallocate `ldr` to match.
    output_format: wgpu::TextureFormat,

    /// The view's motion vectors (#1435): the velocity target the opaque pass writes
    /// while motion blur runs, and last frame's cameras and poses.
    pub(crate) motion: crate::render::motion::MotionVectors,
    /// The motion blur's tile and blurred-scene targets (#1436), while it runs.
    pub(crate) blur: Option<motion_blur::BlurTargets>,
}

/// HDR scene colour format — float so bright pixels survive for bloom/tonemap.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
