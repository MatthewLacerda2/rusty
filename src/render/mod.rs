pub mod mesh;
pub mod shadows;
pub mod skybox;

mod camera;
mod debug_meshes;
mod draw;
mod draw_overlays;
mod draw_pass;
mod draw_path;
mod draw_resources;
mod pipelines;
mod setup;
mod setup_headless;
mod textures;

use std::collections::HashMap;
use std::rc::Rc;

pub use camera::Camera;
pub use setup_headless::OFFSCREEN_FORMAT;

// Represent memory layouts for GPU Uniforms
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [f32; 16],
    camera_pos: [f32; 3],
    _pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AmbientLightUniform {
    color: [f32; 3],
    intensity: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DirectionalLightUniform {
    direction: [f32; 3],
    _pad1: f32,
    color: [f32; 3],
    intensity: f32,
    _pad2: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PointLightUniform {
    position: [f32; 3],
    _pad1: f32,
    color: [f32; 3],
    intensity: f32,
    range: f32,
    _pad2: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SpotlightUniform {
    position: [f32; 3],
    _pad1: f32,
    direction: [f32; 3],
    _pad2: f32,
    color: [f32; 3],
    intensity: f32,
    range: f32,
    inner_cone: f32,
    outer_cone: f32,
    _pad3: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct LightingUniform {
    ambient: AmbientLightUniform,
    dir_light: DirectionalLightUniform,
    point_lights: [PointLightUniform; 4],
    spot_light: SpotlightUniform,
    num_point_lights: u32,
    ssr_active: f32,
    ssr_quality: f32,
    ssr_temporal_upsampling: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EntityUniform {
    model_matrix: [f32; 16],
    color_tint: [f32; 4],
    use_texture: u32,
    is_lit: u32,
    metallic: f32,
    roughness: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BoneUniform {
    bones: [[f32; 16]; 64],
}

// Stores GPU Buffer handlers for meshes
pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

// Stores GPU handlers for textures
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// `None` in the headless (offscreen screenshot) path — there is no window
    /// surface to present to, only an offscreen colour texture.
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,

    // Pipelines
    render_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,

    // Bind Group Layouts
    pub camera_lighting_layout: wgpu::BindGroupLayout,
    pub entity_bones_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,

    // Buffers and Bind Groups (Global)
    camera_buffer: wgpu::Buffer,
    lighting_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,

    // Depth Stencil
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,

    // Asset Cache mapping Entity IDs to GPU buffers
    pub gpu_meshes: HashMap<u32, GpuMesh>,
    pub gpu_textures: HashMap<String, Rc<GpuTexture>>,
    pub default_texture: Rc<GpuTexture>,

    grid_vertex_buffer: Option<wgpu::Buffer>,
    grid_count: u32,

    axis_x_buffer: Option<wgpu::Buffer>,
    axis_y_buffer: Option<wgpu::Buffer>,
    axis_z_buffer: Option<wgpu::Buffer>,
    axis_count: u32,

    pub skybox_renderer: skybox::SkyboxRenderer,
    pub shadow_renderer: shadows::ShadowRenderer,
    pub skybox_texture: Option<Rc<GpuTexture>>,
    pub skybox_path: String,
    pub shadow_layout: wgpu::BindGroupLayout,
    pub shadow_uniform_buffer: wgpu::Buffer,
    pub shadow_bind_group: wgpu::BindGroup,
}
