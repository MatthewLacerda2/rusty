#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "platform layer, exempt from the sim's clock ban (#757): GPU timing is wall-clock"
)]

mod camera;
pub(crate) mod clusters;
mod debug_meshes;
pub(crate) mod decals;
mod draw;
mod frustum;
pub mod gpu;
pub mod ibl;
pub mod interpolate;
pub(crate) mod lod;
pub(crate) mod motion;
pub(crate) mod passes;
pub mod postfx;
mod preview;
pub mod rays;
pub mod readback;
pub(crate) mod render_texture;
pub(crate) mod scene;
mod setup;
mod stats;
#[doc(hidden)]
pub mod test_gpu;
pub(crate) mod timing;
pub mod ui;
mod view;
pub mod warm;

use ibl::{reflection_probes, sky_cube, skybox};
use passes::{particles, shadows};

use rusty_components::MeshRevision;
use std::{collections::HashMap, rc::Rc};

pub use camera::{build_camera_stack, texture_cameras, ScreenStack, TextureCamera};
pub use draw::{NavMeshPatch, NavMeshRevision, NavMeshSource};
pub use frustum::{transform_aabb, Frustum, SkinBounds};
pub(crate) use frustum::{SmallLimits, SmallObjectCull};
pub use gpu::mesh::MeshId;
pub use gpu::ops::STAGING_BUDGET;
pub use gpu::textures::GpuTexture;
pub use ibl::cubemap_capture::{CubemapCapture, CubemapFace};
pub use ibl::probe::bake::{project_cubemap, project_light_probe, DEFAULT_BAKE_RESOLUTION};
pub use ibl::probe::bounce::{BakedDirect, BounceReport, CONVERGENCE_EPSILON, MAX_BOUNCES};
pub use ibl::probe::capture::ProbeCapture;
pub use setup::headless::{gpu_required, ADAPTER_ENV, OFFSCREEN_FORMAT, REQUIRE_GPU_ENV};
pub use setup::SurfaceWindow;
pub use stats::{gpu_memory, GpuMemory, OpCounts, RenderCounters, StaleOcclusion, Uploads};
pub use stats::{EarlyFlushes, RenderSwitch, RenderSwitches};
pub use timing::{ClockProbe, FrameCost, GpuPart, GpuPass, GpuTimes};
pub use view::{PixelSize, RenderView};

pub(crate) use gpu::uniforms::{
    AmbientLightUniform, CameraUniform, DirectionalLightUniform, DrawUniform, InstanceData,
    LightingUniform, MaterialData,
};

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub local_aabb: (glam::Vec3, glam::Vec3),
    pub skin_bounds: SkinBounds,
    pub revision: MeshRevision,
    pub(crate) clusters: Option<crate::passes::hiz::clusters::GpuClusters>,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,

    render_pipeline: gpu::pipelines::FacedPipeline,
    velocity_pipeline: gpu::pipelines::VelocityPipelines,
    primed_pipelines: gpu::pipelines::PrimedPipelines,
    pub(crate) post_passes: postfx::effects::Passes,
    transparent_pipelines: gpu::pipelines::TransparentPipelines,
    depth_postpass_pipelines: gpu::pipelines::DepthPostpassPipelines,
    line_pipeline: gpu::pipelines::LazyPipeline,
    vertex_line_pipeline: gpu::pipelines::LazyPipeline,
    wire_cache: std::cell::RefCell<rusty_physics::WireCache>,
    probe_ball_pipeline: gpu::pipelines::LazyPipeline,
    navmesh: draw::NavMeshGizmo,
    outline: passes::outline::OutlinePasses,

    pub camera_lighting_layout: wgpu::BindGroupLayout,
    pub entity_bones_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,
    pub material_layout: wgpu::BindGroupLayout,

    camera_buffer: draw::camera::buffer::CameraBuffer,
    lighting_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,
    global_bind_group_dirty: bool,
    spare_global_group: ibl::sky_cube::SpareGroup<wgpu::BindGroup>,
    clusters: clusters::ClusterBuffers,
    pub(crate) lightmaps: gpu::lightmaps::Lightmaps,
    draw_buffers: gpu::draw_buffers::DrawBuffers,
    hiz: passes::hiz::HiZ,
    proxy_volumes: draw::proxy_volume::ProxyVolumes,
    unbaked: clusters::UnbakedNote,
    materials: gpu::material_cache::MaterialCache,
    pub(crate) surface_shaders: gpu::pipelines::surface::SurfaceShaders,
    pub(crate) instancing: bool,
    pub(crate) depth_priming: bool,
    pub(crate) priming_recommended: bool,

    pub gpu_meshes: HashMap<MeshId, GpuMesh>,
    pub gpu_textures: HashMap<rusty_asset::MapRef, Rc<GpuTexture>>,
    pub(crate) texture_fallbacks: HashMap<rusty_asset::MapRef, Rc<GpuTexture>>,
    pub(crate) texture_kinds: crate::gpu::textures::kind_watch::KindWatch,
    pub(crate) texture_freshness: rusty_core::fs::freshness::Freshness,
    pub(crate) staging: gpu::ops::Staging,
    pub default_texture: Rc<GpuTexture>,
    pub white_texture: Rc<GpuTexture>,
    pub(crate) mip_generator: gpu::textures::MipGenerator,
    pub default_material_bind_group: wgpu::BindGroup,

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
    reflection_probes: reflection_probes::ProbeArray,
    sky_reflection: sky_cube::SkyReflection,
    probe_prefilter: std::sync::OnceLock<ibl::reflection_prefilter::gpu::ProbePrefilter>,
    pub shadow_layout: wgpu::BindGroupLayout,
    pub shadow_uniform_buffer: wgpu::Buffer,
    pub shadow_bind_group: wgpu::BindGroup,
    pub(crate) opaque_copy: passes::transparent::OpaqueCopyPass,
    pub(crate) prepass_pipeline: gpu::pipelines::PrepassPipelines,
    pub(crate) ssao: passes::ssao::SsaoRenderer,
    pub(crate) grade_bake: postfx::grade::GradeBake,

    pub quality: rusty_core::settings::quality::QualityValues,
    pub graphics_options: rusty_core::settings::graphics_options::GraphicsOptions,
    pub render_scale: rusty_core::settings::render_scale::RenderScale,
    present_modes: Vec<wgpu::PresentMode>,

    particle_renderer: particles::ParticleRenderer,

    ribbon_renderer: passes::ribbons::RibbonRenderer,

    ui_renderer: ui::UiRenderer,

    decals: decals::DecalBuffers,

    fog_volume: postfx::effects::volumetric_fog::SharedVolume,
    linear_clamp: wgpu::Sampler,
    scene_depth_layout: wgpu::BindGroupLayout,

    capture: ibl::cubemap_capture::CaptureMode,
    switches: RenderSwitches,

    pub(crate) retained: scene::Retained,
    pub draw_poses: interpolate::DrawPoses,

    pub frame_counters: RenderCounters,
    pub(crate) uploads: stats::Uploads,
    pub(crate) gpu_timer: timing::GpuTimer,

    render_texture_ids: (HashMap<String, u64>, u64),

    // Declared last: fields drop in order, so the permit frees after the device and every
    // GPU resource.
    _gpu_permit: Option<setup::budget::Permit>,
}
