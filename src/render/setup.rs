use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use super::pipelines;
use super::postfx::{PostFx, QualityPreset, HDR_FORMAT};
use super::{shadows, skybox};
use super::{CameraUniform, LightingUniform, Renderer};

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

        // 4. Configure surface swapchain
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self::from_parts(device, queue, Some(surface), config, size))
    }

    /// Shared constructor body: builds all pipelines, layouts, buffers and bind
    /// groups from an already-created device/queue. Both the windowed (`new`) and
    /// the headless offscreen (`new_headless`) paths funnel through here, so they
    /// produce a byte-identical renderer apart from the optional window surface.
    #[allow(clippy::too_many_lines)] // legacy; burn down in #124
    pub(super) fn from_parts(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> Self {
        // 5. Create Depth Texture
        let (depth_texture, depth_view) = Self::create_depth_resources(&device, &config);

        // 6. Create Bind Group Layouts
        let camera_lighting_layout = pipelines::create_camera_lighting_layout(&device);
        let entity_bones_layout = pipelines::create_entity_bones_layout(&device);
        let texture_layout = pipelines::create_texture_layout(&device);

        // 10. Generate default grid checker texture (moved up to bind statically to global layouts)
        let default_texture = Rc::new(Self::create_default_checkerboard_texture(
            &device,
            &queue,
            &texture_layout,
        ));

        // 7. Create Global Uniform Buffers
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Uniform Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lighting_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lighting Uniform Buffer"),
            size: std::mem::size_of::<LightingUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group"),
            layout: &camera_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&default_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&default_texture.sampler),
                },
            ],
        });

        // 8. Compile WGSL Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Forward Lit Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/shader.wgsl").into(),
            ),
        });

        // Initialize Shadow map system
        let shadow_renderer = shadows::ShadowRenderer::new(&device);

        let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Uniform Buffer"),
            size: 64, // Mat4 size
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_layout = pipelines::create_shadow_layout(&device);

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Main Shadow Bind Group"),
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_renderer.active_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_renderer.sampler),
                },
            ],
        });

        // Initialize Skybox Renderer (draws into the HDR scene target).
        let skybox_renderer = skybox::SkyboxRenderer::new(
            &device,
            &texture_layout,
            &camera_lighting_layout,
            HDR_FORMAT,
        );

        // 9. Create Render Pipelines — the scene draws into the HDR offscreen
        // target now; the post-FX composite is what writes `config.format`.
        let (render_pipeline, line_pipeline, outline_pipeline) = pipelines::create_pipelines(
            &device,
            &shader,
            HDR_FORMAT,
            &camera_lighting_layout,
            &entity_bones_layout,
            &texture_layout,
            &shadow_layout,
        );

        // Post-process chain + default scalability tier.
        let quality = QualityPreset::default();
        let post_fx = PostFx::new(
            &device,
            config.width,
            config.height,
            config.format,
            quality.bloom_divisor(),
        );

        // Billboard particle pass — reuses the texture layout for sprites.
        let particle_renderer = super::particles::ParticleRenderer::new(&device, &texture_layout);

        // Box-projector decal pass — reuses the texture layout for decal sprites.
        let decal_renderer = super::decals::DecalRenderer::new(&device, &texture_layout);

        let mut renderer = Self {
            device,
            queue,
            surface,
            config,
            size,
            render_pipeline,
            line_pipeline,
            outline_pipeline,
            camera_lighting_layout,
            entity_bones_layout,
            texture_layout,
            camera_buffer,
            lighting_buffer,
            global_bind_group,
            depth_texture,
            depth_view,
            gpu_meshes: HashMap::new(),
            gpu_textures: HashMap::new(),
            default_texture,
            grid_vertex_buffer: None,
            grid_count: 0,
            axis_x_buffer: None,
            axis_y_buffer: None,
            axis_z_buffer: None,
            axis_count: 0,
            skybox_renderer,
            shadow_renderer,
            skybox_texture: None,
            skybox_path: "".to_string(),
            shadow_layout,
            shadow_uniform_buffer,
            shadow_bind_group,
            post_fx,
            quality,
            particle_renderer,
            decal_renderer,
        };

        renderer.generate_grid_mesh();
        renderer.generate_axis_arrows();
        renderer
    }
}
