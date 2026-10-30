pub(crate) mod budget;
pub(crate) mod build;
pub(crate) mod gpu_rule;
pub(crate) mod headless;
pub(crate) mod resize;
mod shadow_system;
pub(crate) mod textures;
mod warm;

use std::collections::HashMap;

use self::build::GpuResources;
use crate::render::{PixelSize, Renderer};

/// What [`Renderer::new`] presents to: a window wgpu can build a surface from, whose
/// display handle also goes to the instance. `shell` hands in its `Arc` of the OS
/// window, so render never names the windowing crate (#988).
pub trait SurfaceWindow:
    wgpu::WindowHandle + wgpu::rwh::HasDisplayHandle + std::fmt::Debug + Clone + 'static
{
}
impl<W> SurfaceWindow for W where
    W: wgpu::WindowHandle + wgpu::rwh::HasDisplayHandle + std::fmt::Debug + Clone + 'static
{
}

impl Renderer {
    /// Build the windowed renderer. Returns `Err` with a human-readable message
    /// when surface/adapter/device acquisition fails (unsupported or broken GPU
    /// drivers), so `main` can report it cleanly and exit instead of unwinding
    /// through a panic + backtrace. Mirrors the graceful headless path in
    /// `setup_headless.rs`. `size` is the window's inner size in physical pixels.
    pub async fn new(window: impl SurfaceWindow, size: PixelSize) -> Result<Self, String> {
        // 1. Create wgpu Instance
        // The window's display handle goes in with the instance: the GL backend
        // needs it to present (Wayland especially); Vulkan and Metal ignore it.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
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
                // BC compression where the GPU has it (#1574).
                required_features: adapter.features()
                    & crate::render::gpu::textures::compressed::FEATURE,
                required_limits: wgpu::Limits::default(),
                label: None,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("failed to create a GPU device: {e}"))?;

        // 4. Configure surface swapchain.
        let surface_caps = surface.get_capabilities(&adapter);
        let mut config = surface_config(&surface_caps, size);
        // Let the UI pass view the sRGB swapchain as its non-sRGB twin, so a HUD
        // blends in display space on the player's window too (#418).
        let flags = adapter.get_downlevel_capabilities().flags;
        let alias = config.format.remove_srgb_suffix();
        if flags.contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS) && alias != config.format {
            config.view_formats = vec![alias];
        }
        surface.configure(&device, &config);

        let present_modes = surface_caps.present_modes.clone();
        // No budget permit: there is exactly one windowed renderer and it is the
        // application itself, so it never queues behind the headless budget (#366).
        let renderer = Self::from_parts(device, queue, Some(surface), config, present_modes, None);
        // A game must not compile a pipeline mid-frame; only headless stays lazy (#985).
        renderer.warm_pipelines(cfg!(feature = "editor"));
        Ok(renderer)
    }

    /// Shared constructor body: builds all pipelines, layouts, buffers and bind
    /// groups from an already-created device/queue. Both the windowed (`new`) and
    /// the headless offscreen (`new_headless`) paths funnel through here, so they
    /// produce a byte-identical renderer apart from the optional window surface;
    /// only `new` then warms the pipelines built on first draw (#985).
    ///
    /// `gpu_permit` is the caller's slot in the headless budget (#366): `Some` from the
    /// headless path, `None` from the windowed one. It is merely *stored* here — it must
    /// already have been acquired before the device was created, since the point of the
    /// budget is to bound live GPU allocations, not constructor calls.
    pub(crate) fn from_parts(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        present_modes: Vec<wgpu::PresentMode>,
        gpu_permit: Option<budget::Permit>,
    ) -> Self {
        // Build every shared GPU resource up-front, grouped by role (textures, global
        // bindings, shadow system, forward/billboard passes), then assemble the
        // flat renderer from those groups and seed the editor gizmo meshes. Per-view
        // targets (depth + post-FX) belong to each `RenderView`, not the renderer (#355).
        let gpu = GpuResources::build(&device, &queue, config.format);
        let mut renderer = Self::assemble(
            device,
            queue,
            surface,
            config,
            present_modes,
            gpu,
            gpu_permit,
        );
        renderer.generate_grid_mesh();
        renderer.generate_axis_arrows();
        renderer
    }

    /// Flatten the grouped [`GpuResources`] plus the window-dependent parameters
    /// into the renderer's single struct. Trivial fields (the asset caches, the
    /// editor gizmo buffers, the skybox slots) start empty/`None` and are filled in
    /// by the caller or at runtime.
    // A flat field-mapping constructor: no logic, just one line per `Renderer`
    // field, so the line/argument counts are inherent to the struct's width.
    #[expect(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "a flat one-line-per-field constructor: its length is the struct's width (docs/linting.md)"
    )]
    fn assemble(
        device: wgpu::Device,
        queue: wgpu::Queue,
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
            crate::render::gpu::draw_buffers::DrawBuffers::new(&device, &entity_bones_layout);
        let gpu_timer = crate::render::timing::GpuTimer::new(&device, &queue);
        let surface_shaders = crate::render::gpu::pipelines::surface::SurfaceShaders::new(
            crate::render::gpu::pipelines::forward_layout(
                &device,
                [
                    &camera_lighting_layout,
                    &entity_bones_layout,
                    &textures.material_layout,
                    &shadows.layout,
                ],
            ),
            shadows
                .renderer
                .clip_layout(&device, &textures.material_layout),
            crate::render::postfx::HDR_FORMAT,
        );
        // Depth priming follows the adapter's backend (#1499): never on Metal.
        let backend = device.adapter_info().backend;
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
            outline: forward.outline,
            prepass_pipeline: forward.prepass_pipeline,
            ssao,
            grade_bake,
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
            clusters: global.clusters,
            lightmaps: global.lightmaps,
            draw_buffers,
            materials: crate::render::gpu::material_cache::MaterialCache::new(textures.zero_params),
            surface_shaders,
            instancing: true,
            depth_priming: true,
            priming_recommended: crate::render::gpu::pipelines::priming::recommended(backend),
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
            texture_freshness: crate::core::fs::freshness::Freshness::new(),
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
            capture: Default::default(),
            frame_counters: Default::default(),
            gpu_timer,
            render_texture_ids: Default::default(),
            _gpu_permit: gpu_permit,
        }
    }
}

/// Build the swapchain config for a window of `size` from the surface caps: prefer
/// an sRGB format, and boot with vsync on (`Fifo`, supported on every backend).
/// Vsync is toggled later via [`Renderer::set_vsync`] (issue #89).
fn surface_config(caps: &wgpu::SurfaceCapabilities, size: PixelSize) -> wgpu::SurfaceConfiguration {
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or_else(|| {
            // The target format owns the one linear→display encode (#415); without an
            // sRGB surface the window shows linear colour, darker than a screenshot.
            log::warn!("[Render] no sRGB surface format; the window will not match screenshots");
            caps.formats[0]
        });
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        color_space: wgpu::SurfaceColorSpace::Auto,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}
