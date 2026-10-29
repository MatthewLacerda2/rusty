//! src/render/postfx/mod.rs — Post-process chain: the post effects (`effects`, one
//! module per effect, #1467), the grading LUT they bake into (`grade`) and the chain
//! that runs them (`run`).
//!
//! The scene no longer renders straight to the swapchain. It renders into an HDR
//! offscreen target; this module then runs the post-FX chain (the effects' scene
//! passes, the assembled composite, the LDR passes) that finally reads the
//! `PostProcessingComponent` / `CameraComponent` knobs and writes the corrected image
//! to the real target.
//!
//! The running quality preset's values
//! ([`QualityValues`](crate::core::settings::quality::QualityValues), sim-side, #1522)
//! set what each effect's quality level costs (bloom's buffer size, SSAO's samples,
//! SSR's march, ...), so an integrated GPU can hold ~30fps; no preset turns a pass off.

pub(crate) mod effects;
pub(crate) mod grade;
mod inputs;
pub(crate) mod params;
mod run;
mod setup;

pub use inputs::PostFxContext;

use glam::Mat4;

/// CPU-side mirror of the `PostParams` uniform in `postfx/params.wgsl`: the frame
/// every post pass shares. Each effect's own numbers are its slots of the
/// composite's effect array (`effects::composite_slots`, #1467).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostParams {
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
    /// The composite, assembled from the effects' functions (`effects::assemble`).
    pub composite_pipeline: wgpu::RenderPipeline,
    /// Final anti-aliasing pass (#360), reading [`PostFx::ldr`] and writing the output.
    pub fxaa_pipeline: wgpu::RenderPipeline,
    /// Plain copy, landing the authored-effect chain's result in the output when
    /// FXAA is off (#397).
    pub copy_pipeline: wgpu::RenderPipeline,
    /// The grade + tonemap, baked into a 3D LUT the composite samples (#1440).
    pub grade: grade::GradeLut,
    /// The view's automatic-exposure meter and its adapted EV (#1444).
    pub exposure: effects::exposure::ExposureMeter,
    /// Authored post-FX effects (#397): their module cache, second LDR target and
    /// previous-frame history.
    pub custom: effects::custom_effects::CustomChain,
    pub io_layout: wgpu::BindGroupLayout,
    /// The composite's group 1, the effects' own textures (#1473).
    pub(crate) effect_textures: effects::textures::CompositeTextures,

    pub params_buffer: wgpu::Buffer,
    /// The composite's effect slots (binding 9), packed every frame.
    pub fx_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    /// HDR scene colour the main pass draws into.
    pub scene_hdr: PfxTarget,
    /// Bloom's ping-pong buffers (bright pass / blur), at reduced resolution.
    pub(crate) bloom: effects::bloom::BloomTargets,
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
    pub full_size: (u32, u32),
    /// The output format `ldr` and the composite/FXAA pipelines are built against,
    /// kept so `resize` can reallocate `ldr` to match.
    output_format: wgpu::TextureFormat,

    /// The view's motion vectors (#1435): the velocity target the opaque pass writes
    /// while motion blur runs, and last frame's cameras and poses.
    pub(crate) motion: crate::render::motion::MotionVectors,
    /// Each effect's own targets while it runs (#1467): depth of field's (#1445),
    /// motion blur's tiles and blurred scene (#1436), one slot per effect.
    pub(crate) targets: effects::Targets,
}

/// HDR scene colour format — float so bright pixels survive for bloom/tonemap.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
