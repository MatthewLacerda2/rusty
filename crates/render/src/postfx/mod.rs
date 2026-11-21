pub(crate) mod effects;
#[doc(hidden)]
pub use effects::completeness;
pub(crate) mod grade;
mod inputs;
pub(crate) mod params;
mod reads;
mod run;
mod setup;

pub use inputs::PostFxContext;

use glam::Mat4;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostParams {
    pub inv_view_proj: [f32; 16],
    pub view_proj: [f32; 16],
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

pub struct PfxTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

pub struct PostFx {
    pub composite_pipeline: wgpu::RenderPipeline,
    pub fxaa_pipeline: wgpu::RenderPipeline,
    pub copy_pipeline: wgpu::RenderPipeline,
    pub grade: grade::GradeLut,
    pub exposure: effects::exposure::ExposureMeter,
    pub custom: effects::custom_effects::CustomChain,
    pub io_layout: wgpu::BindGroupLayout,
    pub(crate) effect_textures: effects::textures::CompositeTextures,

    pub params_buffer: wgpu::Buffer,
    pub fx_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    pub scene_hdr: PfxTarget,
    pub(crate) bloom_dirt: Option<std::rc::Rc<crate::GpuTexture>>,
    pub ldr: PfxTarget,
    pub full_size: (u32, u32),
    pub(crate) chain_size: (u32, u32),
    output_format: wgpu::TextureFormat,

    pub(crate) motion: crate::motion::MotionVectors,
    pub(crate) targets: effects::Targets,
}

pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
