pub(crate) mod budget;
pub(crate) mod build;
pub(crate) mod gpu_rule;
pub(crate) mod headless;
mod limits;
pub(crate) mod resize;
mod shadow_system;
pub(crate) mod textures;
mod warm;

use self::build::GpuResources;
use crate::{PixelSize, RenderSwitches, Renderer};
use std::collections::HashMap;

pub trait SurfaceWindow:
    wgpu::WindowHandle + wgpu::rwh::HasDisplayHandle + std::fmt::Debug + Clone + 'static
{
}
impl<W> SurfaceWindow for W where
    W: wgpu::WindowHandle + wgpu::rwh::HasDisplayHandle + std::fmt::Debug + Clone + 'static
{
}

impl Renderer {
    pub async fn new(window: impl SurfaceWindow, size: PixelSize) -> Result<Self, String> {
        // 1. The instance takes the window's display handle: GL needs it to present (Wayland
        // especially).
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            // GL too, for a machine with nothing else; only headless leaves it out.
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(window.clone()))
        });

        // 2. Create Surface & Adapter
        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("failed to create a render surface for the window: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| {
                format!("no compatible GPU adapter found (check your graphics drivers): {e}")
            })?;

        // 3. Create Device & Queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: adapter.features() & limits::WINDOW,
                required_limits: limits::device_limits(),
                label: None,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("failed to create a GPU device: {e}"))?;

        // 4. Configure surface swapchain.
        let surface_caps = surface.get_capabilities(&adapter);
        let mut config = resize::surface_config(&surface_caps, size);
        let flags = adapter.get_downlevel_capabilities().flags;
        let alias = config.format.remove_srgb_suffix();
        if flags.contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS) && alias != config.format {
            config.view_formats = vec![alias];
        }
        surface.configure(&device, &config);

        let present_modes = surface_caps.present_modes.clone();
        let mut renderer =
            Self::from_parts(device, queue, Some(surface), config, present_modes, None);
        renderer.warm_pipelines(cfg!(feature = "editor"));
        Ok(renderer)
    }

    pub(crate) fn from_parts(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        present_modes: Vec<wgpu::PresentMode>,
        gpu_permit: Option<budget::Permit>,
    ) -> Self {
        let staging = crate::gpu::ops::Staging::new(&device);
        let gpu = GpuResources::build(&device, (&queue, &staging), config.format);
        let mut renderer = Self::assemble(
            device,
            (queue, staging),
            surface,
            config,
            present_modes,
            gpu,
            gpu_permit,
        );
        // Headless reads lightmap pages in line (a lit first frame); a window on a worker.
        renderer.lightmaps.read_in_line(renderer.surface.is_none());
        renderer.generate_grid_mesh();
        renderer.generate_axis_arrows();
        renderer
    }

    // A flat field-mapping constructor: no logic, just one line per `Renderer`
    // field, so the line/argument counts are inherent to the struct's width.
    #[expect(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "a flat one-line-per-field constructor: its length is the struct's width (docs/linting.md)"
    )]
    fn assemble(
        device: wgpu::Device,
        (queue, staging): (wgpu::Queue, crate::gpu::ops::Staging),
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        present_modes: Vec<wgpu::PresentMode>,
        gpu: GpuResources,
        gpu_permit: Option<budget::Permit>,
    ) -> Self {
        let GpuResources {
            camera_lighting_layout,
            entity_bones_layout,
            textures,
            global,
            shadows,
            forward,
            billboards,
            ui,
            ssao,
            grade_bake,
            post_passes,
            quality,
        } = gpu;
        let draw_buffers =
            crate::gpu::draw_buffers::DrawBuffers::new(&device, &entity_bones_layout);
        let gpu_timer = crate::timing::GpuTimer::new(&device, &queue);
        let surface_shaders = crate::gpu::pipelines::surface::SurfaceShaders::new(
            crate::gpu::pipelines::forward_layout(
                &device,
                crate::gpu::groups::Groups {
                    global: &camera_lighting_layout,
                    scene: Some(&shadows.layout),
                    material: Some(&textures.material_layout),
                    draw: &entity_bones_layout,
                },
            ),
            shadows
                .renderer
                .clip_layout(&device, &textures.material_layout),
            crate::postfx::HDR_FORMAT,
        );
        let backend = device.adapter_info().backend;
        let hiz = crate::passes::hiz::HiZ::new(&device);
        Self {
            device,
            queue,
            surface,
            config,
            present_modes,
            render_pipeline: forward.render_pipeline,
            velocity_pipeline: forward.velocity_pipeline,
            primed_pipelines: forward.primed_pipelines,
            post_passes,
            transparent_pipelines: forward.transparent_pipelines,
            depth_postpass_pipelines: forward.depth_postpass_pipelines,
            line_pipeline: forward.line_pipeline,
            vertex_line_pipeline: forward.vertex_line_pipeline,
            wire_cache: Default::default(),
            probe_ball_pipeline: forward.probe_ball_pipeline,
            navmesh: forward.navmesh,
            outline: forward.outline,
            prepass_pipeline: forward.prepass_pipeline,
            ssao,
            grade_bake,
            ssr_rays: Default::default(),
            skybox_renderer: forward.skybox_renderer,
            camera_lighting_layout,
            entity_bones_layout,
            texture_layout: textures.texture_layout,
            material_layout: textures.material_layout,
            default_texture: textures.default_texture,
            white_texture: textures.white_texture,
            mip_generator: textures.mip_generator,
            default_material_bind_group: textures.default_material_bind_group,
            camera_buffer: global.camera_buffer,
            lighting_buffer: global.lighting_buffer,
            global_bind_group: global.global_bind_group,
            global_bind_group_dirty: false,
            spare_global_group: Default::default(),
            clusters: global.clusters,
            lightmaps: global.lightmaps,
            fog_volume: global.fog_volume,
            linear_clamp: global.linear_clamp,
            draw_buffers,
            hiz,
            proxy_volumes: Default::default(),
            unbaked: Default::default(),
            materials: crate::gpu::material_cache::MaterialCache::new(textures.zero_params),
            surface_shaders,
            instancing: true,
            depth_priming: true,
            priming_recommended: crate::gpu::pipelines::priming::recommended(backend),
            shadow_layout: shadows.layout,
            shadow_renderer: shadows.renderer,
            shadow_uniform_buffer: shadows.uniform_buffer,
            shadow_bind_group: shadows.bind_group,
            opaque_copy: shadows.opaque_copy,
            particle_renderer: billboards.particle_renderer,
            ribbon_renderer: billboards.ribbon_renderer,
            decals: global.decals,
            scene_depth_layout: billboards.scene_depth_layout,
            ui_renderer: ui,
            quality,
            graphics_options: Default::default(),
            render_scale: Default::default(),
            gpu_meshes: HashMap::new(),
            gpu_textures: HashMap::new(),
            texture_fallbacks: HashMap::new(),
            texture_kinds: Default::default(),
            texture_freshness: rusty_core::fs::freshness::Freshness::new(),
            staging,
            grid_vertex_buffer: None,
            grid_count: 0,
            axis_x_buffer: None,
            axis_y_buffer: None,
            axis_z_buffer: None,
            axis_count: 0,
            skybox_texture: None,
            skybox_path: "".to_string(),
            reflection_probes: global.reflection_probes,
            sky_reflection: global.sky_reflection,
            probe_prefilter: Default::default(),
            capture: Default::default(),
            switches: RenderSwitches::ON,
            retained: Default::default(),
            draw_poses: Default::default(),
            frame_counters: Default::default(),
            uploads: Default::default(),
            gpu_timer,
            render_texture_ids: Default::default(),
            _gpu_permit: gpu_permit,
        }
    }
}
