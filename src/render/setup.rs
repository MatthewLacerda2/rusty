use std::collections::HashMap;
use std::sync::Arc;

use super::setup_build::RendererParts;
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
        // All GPU resources (depth, layouts, buffers, bind groups, pipelines and
        // the auxiliary passes) are built up-front in `build_parts`; here we only
        // move them into the flat `Renderer` and seed the editor gizmo meshes.
        let p = RendererParts::build(&device, &queue, &config);

        let mut renderer = Self {
            device,
            queue,
            surface,
            config,
            size,
            render_pipeline: p.render_pipeline,
            line_pipeline: p.line_pipeline,
            outline_pipeline: p.outline_pipeline,
            camera_lighting_layout: p.camera_lighting_layout,
            entity_bones_layout: p.entity_bones_layout,
            texture_layout: p.texture_layout,
            material_layout: p.material_layout,
            camera_buffer: p.camera_buffer,
            lighting_buffer: p.lighting_buffer,
            global_bind_group: p.global_bind_group,
            depth_texture: p.depth_texture,
            depth_view: p.depth_view,
            gpu_meshes: HashMap::new(),
            gpu_textures: HashMap::new(),
            default_texture: p.default_texture,
            default_material_bind_group: p.default_material_bind_group,
            grid_vertex_buffer: None,
            grid_count: 0,
            axis_x_buffer: None,
            axis_y_buffer: None,
            axis_z_buffer: None,
            axis_count: 0,
            skybox_renderer: p.skybox_renderer,
            shadow_renderer: p.shadow_renderer,
            skybox_texture: None,
            skybox_path: "".to_string(),
            shadow_layout: p.shadow_layout,
            shadow_uniform_buffer: p.shadow_uniform_buffer,
            shadow_bind_group: p.shadow_bind_group,
            post_fx: p.post_fx,
            quality: p.quality,
            present_modes,
            particle_renderer: p.particle_renderer,
            decal_renderer: p.decal_renderer,
        };

        renderer.generate_grid_mesh();
        renderer.generate_axis_arrows();
        renderer
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
