use std::collections::HashMap;
use std::sync::Arc;

use super::setup_build::GpuResources;
use super::Renderer;

impl Renderer {
    /// Build the windowed renderer. Returns `Err` with a human-readable message
    /// when surface/adapter/device acquisition fails (unsupported or broken GPU
    /// drivers), so `main` can report it cleanly and exit instead of unwinding
    /// through a panic + backtrace. Mirrors the graceful headless path in
    /// `setup_headless.rs`.
    pub async fn new(window: Arc<winit::window::Window>) -> Result<Self, String> {
        let size = window.inner_size();

        // 1. Create wgpu Instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
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
            })
            .await
            .ok_or_else(|| {
                "no compatible GPU adapter found (check your graphics drivers)".to_string()
            })?;

        // 3. Create Device & Queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .map_err(|e| format!("failed to create a GPU device: {e}"))?;

        // 4. Configure surface swapchain.
        let surface_caps = surface.get_capabilities(&adapter);
        let config = surface_config(&surface_caps, size);
        surface.configure(&device, &config);

        let present_modes = surface_caps.present_modes.clone();
        Ok(Self::from_parts(
            device,
            queue,
            Some(surface),
            config,
            size,
            present_modes,
        ))
    }

    /// Shared constructor body: builds all pipelines, layouts, buffers and bind
    /// groups from an already-created device/queue. Both the windowed (`new`) and
    /// the headless offscreen (`new_headless`) paths funnel through here, so they
    /// produce a byte-identical renderer apart from the optional window surface.
    pub(super) fn from_parts(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        size: winit::dpi::PhysicalSize<u32>,
        present_modes: Vec<wgpu::PresentMode>,
    ) -> Self {
        // Build every GPU resource up-front, grouped by role (textures, global
        // bindings, shadow system, forward/billboard passes), then assemble the
        // flat renderer from those groups and seed the editor gizmo meshes.
        let gpu = GpuResources::build(&device, &queue, &config);
        let mut renderer = Self::assemble(device, queue, surface, config, size, present_modes, gpu);
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
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn assemble(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        size: winit::dpi::PhysicalSize<u32>,
        present_modes: Vec<wgpu::PresentMode>,
        gpu: GpuResources,
    ) -> Self {
        let GpuResources {
            depth_texture,
            depth_view,
            camera_lighting_layout,
            entity_bones_layout,
            textures,
            global,
            shadows,
            forward,
            billboards,
            post_fx,
            quality,
        } = gpu;
        let entity_pool = Some(super::entity_pool::EntityPool::new(&device));
        Self {
            device,
            queue,
            surface,
            config,
            size,
            present_modes,
            render_pipeline: forward.render_pipeline,
            line_pipeline: forward.line_pipeline,
            outline_pipeline: forward.outline_pipeline,
            skybox_renderer: forward.skybox_renderer,
            camera_lighting_layout,
            entity_bones_layout,
            texture_layout: textures.texture_layout,
            material_layout: textures.material_layout,
            default_texture: textures.default_texture,
            default_material_bind_group: textures.default_material_bind_group,
            camera_buffer: global.camera_buffer,
            lighting_buffer: global.lighting_buffer,
            global_bind_group: global.global_bind_group,
            global_bind_group_dirty: false,
            entity_pool,
            depth_texture,
            depth_view,
            viewport_target: None,
            shadow_layout: shadows.layout,
            shadow_renderer: shadows.renderer,
            shadow_uniform_buffer: shadows.uniform_buffer,
            shadow_bind_group: shadows.bind_group,
            particle_renderer: billboards.particle_renderer,
            decal_renderer: billboards.decal_renderer,
            post_fx,
            quality,
            gpu_meshes: HashMap::new(),
            gpu_textures: HashMap::new(),
            grid_vertex_buffer: None,
            grid_count: 0,
            axis_x_buffer: None,
            axis_y_buffer: None,
            axis_z_buffer: None,
            axis_count: 0,
            skybox_texture: None,
            skybox_path: "".to_string(),
            decal_depth_bind_group: None,
            static_capture: false,
        }
    }
}

/// Build the swapchain config for a window of `size` from the surface caps: prefer
/// an sRGB format, and boot with vsync on (`Fifo`, supported on every backend).
/// Vsync is toggled later via [`Renderer::set_vsync`] (issue #89).
fn surface_config(
    caps: &wgpu::SurfaceCapabilities,
    size: winit::dpi::PhysicalSize<u32>,
) -> wgpu::SurfaceConfiguration {
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}
