pub mod mesh;
pub mod skybox;
pub mod shadows;

use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::HashMap;
use wgpu::util::DeviceExt;
use glam::{Vec3, Mat4, Quat};
use image::GenericImageView;

use self::mesh::Vertex;
use crate::core::scene::{Scene, Entity, LightType, LightComponent};

// Camera representation
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // Degrees (rotation around Y axis)
    pub pitch: f32,  // Degrees (rotation around X axis)
}

impl Camera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self { position, yaw, pitch }
    }

    pub fn forward(&self) -> Vec3 {
        let pitch_rad = self.pitch.to_radians();
        let yaw_rad = self.yaw.to_radians();
        
        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        Vec3::new(
            cos_yaw * cos_pitch,
            sin_pitch,
            sin_yaw * cos_pitch
        ).normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn build_view_projection(&self, aspect: f32) -> Mat4 {
        let forward = self.forward();
        let view = Mat4::look_at_rh(self.position, self.position + forward, Vec3::Y);
        let proj = Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 200.0);
        proj * view
    }
}

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
    pub surface: wgpu::Surface<'static>,
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

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();

        // 1. Create wgpu Instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // 2. Create Surface & Adapter
        let surface = instance.create_surface(window).unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.expect("Failed to find wgpu adapter");

        // 3. Create Device & Queue
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.expect("Failed to create wgpu device");

        // 4. Configure surface swapchain
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
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

        // 5. Create Depth Texture
        let (depth_texture, depth_view) = Self::create_depth_resources(&device, &config);

        // 6. Create Bind Group Layouts
        // Group 0: Camera (0), Lighting (1), Skybox Texture (2), Skybox Sampler (3)
        let camera_lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera, Lighting & Skybox Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Group 1: Entity model matrix + color tint (0) & Bone Matrices (1)
        let entity_bones_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Entity & Bones Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Group 2: Texture (0) & Sampler (1)
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 10. Generate default grid checker texture (moved up to bind statically to global layouts)
        let default_texture = Rc::new(Self::create_default_checkerboard_texture(&device, &queue, &texture_layout));

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
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/shader.wgsl").into()),
        });

        // Initialize Shadow map system
        let shadow_renderer = shadows::ShadowRenderer::new(&device);

        let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Uniform Buffer"),
            size: 64, // Mat4 size
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Main Shadow Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

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

        // Initialize Skybox Renderer
        let skybox_renderer = skybox::SkyboxRenderer::new(
            &device,
            &texture_layout,
            &camera_lighting_layout,
            surface_format,
        );

        // 9. Create Render Pipelines
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &camera_lighting_layout,
                &entity_bones_layout,
                &texture_layout,
                &shadow_layout,
            ],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Forward Lit Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling to easily render primitives inside-out if needed
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Line debug pipeline layout
        let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Line Pipeline Layout"),
            bind_group_layouts: &[
                &camera_lighting_layout,
                &entity_bones_layout,
                &texture_layout,
                &shadow_layout,
            ],
            push_constant_ranges: &[],
        });

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Line Debug Pipeline"),
            layout: Some(&line_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Don't write to depth buffer for line overlays so they display over grid
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Outline pipeline for selection silhouette (inverted hull technique)
        let outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Outline Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front), // Cull front faces so only back faces (the outline) show
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });



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
        };

        renderer.generate_grid_mesh();
        renderer.generate_axis_arrows();
        renderer
    }

    fn create_depth_resources(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            let (depth_tex, depth_view) = Self::create_depth_resources(&self.device, &self.config);
            self.depth_texture = depth_tex;
            self.depth_view = depth_view;
        }
    }

    /// Generates a standard checkerboard texture for meshes that don't have texture files assigned
    fn create_default_checkerboard_texture(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout) -> GpuTexture {
        let width = 64;
        let height = 64;
        let mut pixels = Vec::with_capacity(width * height * 4);

        // Generate glowing dark neon checker pattern
        for y in 0..height {
            for x in 0..width {
                let is_even = ((x / 8) + (y / 8)) % 2 == 0;
                let (r, g, b) = if is_even {
                    (28, 30, 42) // Dark violet grey
                } else {
                    (52, 45, 78) // Glowing neon violet
                };
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
                pixels.push(255);
            }
        }

        let size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Checkerboard Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width as u32),
                rows_per_image: Some(height as u32),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Default Checkerboard Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        GpuTexture { texture, view, sampler, bind_group }
    }

    /// Loads and registers a new texture from a filepath. Caches results dynamically.
    pub fn load_texture(&mut self, path_str: &str) -> Rc<GpuTexture> {
        if let Some(tex) = self.gpu_textures.get(path_str) {
            return Rc::clone(tex);
        }

        // Try load texture
        let tex = match image::open(path_str) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let dimensions = img.dimensions();

                let size = wgpu::Extent3d {
                    width: dimensions.0,
                    height: dimensions.1,
                    depth_or_array_layers: 1,
                };

                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(path_str),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                self.queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &rgba,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * dimensions.0),
                        rows_per_image: Some(dimensions.1),
                    },
                    size,
                );

                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    anisotropy_clamp: 16,
                    ..Default::default()
                });

                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(path_str),
                    layout: &self.texture_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });

                Rc::new(GpuTexture { texture, view, sampler, bind_group })
            }
            Err(_) => {
                // Fallback to default
                Rc::clone(&self.default_texture)
            }
        };

        self.gpu_textures.insert(path_str.to_string(), Rc::clone(&tex));
        tex
    }

    /// Uploads mesh data to GPU and caches under Entity ID
    pub fn update_gpu_mesh(&mut self, entity_id: u32, vertices: &[Vertex], indices: &[u32]) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Mesh Vertices (Entity {})", entity_id)),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Mesh Indices (Entity {})", entity_id)),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.gpu_meshes.insert(entity_id, GpuMesh {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        });
    }

    /// Pre-generates the static 3D line grid mesh for Editor visual feedback
    fn generate_grid_mesh(&mut self) {
        let mut vertices = Vec::new();
        let spacing = 2.0;
        let extent = 30.0;
        
        let mut count = 0;
        let normal = Vec3::Y;
        let uv = [0.0, 0.0];

        // Draw horizontal and vertical lines in XZ plane
        let mut x = -extent;
        while x <= extent {
            vertices.push(Vertex::new(Vec3::new(x, 0.0, -extent), normal, uv));
            vertices.push(Vertex::new(Vec3::new(x, 0.0,  extent), normal, uv));
            
            vertices.push(Vertex::new(Vec3::new(-extent, 0.0, x), normal, uv));
            vertices.push(Vertex::new(Vec3::new( extent, 0.0, x), normal, uv));
            
            x += spacing;
            count += 4;
        }

        self.grid_vertex_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Mesh Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        self.grid_count = count;
    }

    /// Pre-generates 3D line-based arrows for X, Y, Z global translation axes
    fn generate_axis_arrows(&mut self) {
        let axis_length = 2.0;
        let arrow_head_length = 0.35;
        let arrow_head_width = 0.12;
        let uv = [0.0, 0.0];
        let normal = Vec3::Y;

        // X Axis (Red)
        let mut x_verts = Vec::new();
        let t_x = Vec3::new(axis_length, 0.0, 0.0);
        let s_x = Vec3::new(axis_length - arrow_head_length, 0.0, 0.0);
        x_verts.push(Vertex::new(Vec3::ZERO, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv));
        // Arrowhead
        let a_x1 = s_x + Vec3::new(0.0, arrow_head_width, 0.0);
        let a_x2 = s_x - Vec3::new(0.0, arrow_head_width, 0.0);
        let a_x3 = s_x + Vec3::new(0.0, 0.0, arrow_head_width);
        let a_x4 = s_x - Vec3::new(0.0, 0.0, arrow_head_width);
        x_verts.push(Vertex::new(t_x, normal, uv)); x_verts.push(Vertex::new(a_x1, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv)); x_verts.push(Vertex::new(a_x2, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv)); x_verts.push(Vertex::new(a_x3, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv)); x_verts.push(Vertex::new(a_x4, normal, uv));
        x_verts.push(Vertex::new(a_x1, normal, uv)); x_verts.push(Vertex::new(a_x3, normal, uv));
        x_verts.push(Vertex::new(a_x3, normal, uv)); x_verts.push(Vertex::new(a_x2, normal, uv));
        x_verts.push(Vertex::new(a_x2, normal, uv)); x_verts.push(Vertex::new(a_x4, normal, uv));
        x_verts.push(Vertex::new(a_x4, normal, uv)); x_verts.push(Vertex::new(a_x1, normal, uv));

        // Y Axis (Green)
        let mut y_verts = Vec::new();
        let t_y = Vec3::new(0.0, axis_length, 0.0);
        let s_y = Vec3::new(0.0, axis_length - arrow_head_length, 0.0);
        y_verts.push(Vertex::new(Vec3::ZERO, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv));
        // Arrowhead
        let a_y1 = s_y + Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_y2 = s_y - Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_y3 = s_y + Vec3::new(0.0, 0.0, arrow_head_width);
        let a_y4 = s_y - Vec3::new(0.0, 0.0, arrow_head_width);
        y_verts.push(Vertex::new(t_y, normal, uv)); y_verts.push(Vertex::new(a_y1, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv)); y_verts.push(Vertex::new(a_y2, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv)); y_verts.push(Vertex::new(a_y3, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv)); y_verts.push(Vertex::new(a_y4, normal, uv));
        y_verts.push(Vertex::new(a_y1, normal, uv)); y_verts.push(Vertex::new(a_y3, normal, uv));
        y_verts.push(Vertex::new(a_y3, normal, uv)); y_verts.push(Vertex::new(a_y2, normal, uv));
        y_verts.push(Vertex::new(a_y2, normal, uv)); y_verts.push(Vertex::new(a_y4, normal, uv));
        y_verts.push(Vertex::new(a_y4, normal, uv)); y_verts.push(Vertex::new(a_y1, normal, uv));

        // Z Axis (Blue)
        let mut z_verts = Vec::new();
        let t_z = Vec3::new(0.0, 0.0, axis_length);
        let s_z = Vec3::new(0.0, 0.0, axis_length - arrow_head_length);
        z_verts.push(Vertex::new(Vec3::ZERO, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv));
        // Arrowhead
        let a_z1 = s_z + Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_z2 = s_z - Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_z3 = s_z + Vec3::new(0.0, arrow_head_width, 0.0);
        let a_z4 = s_z - Vec3::new(0.0, -arrow_head_width, 0.0);
        z_verts.push(Vertex::new(t_z, normal, uv)); z_verts.push(Vertex::new(a_z1, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv)); z_verts.push(Vertex::new(a_z2, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv)); z_verts.push(Vertex::new(a_z3, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv)); z_verts.push(Vertex::new(a_z4, normal, uv));
        z_verts.push(Vertex::new(a_z1, normal, uv)); z_verts.push(Vertex::new(a_z3, normal, uv));
        z_verts.push(Vertex::new(a_z3, normal, uv)); z_verts.push(Vertex::new(a_z2, normal, uv));
        z_verts.push(Vertex::new(a_z2, normal, uv)); z_verts.push(Vertex::new(a_z4, normal, uv));
        z_verts.push(Vertex::new(a_z4, normal, uv)); z_verts.push(Vertex::new(a_z1, normal, uv));

        self.axis_x_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Axis X Buffer"),
            contents: bytemuck::cast_slice(&x_verts),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        self.axis_y_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Axis Y Buffer"),
            contents: bytemuck::cast_slice(&y_verts),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        self.axis_z_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Axis Z Buffer"),
            contents: bytemuck::cast_slice(&z_verts),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        self.axis_count = x_verts.len() as u32;
    }

    /// Renders the 3D scene inside a viewport render pass
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        view_texture: &wgpu::TextureView,
        editor_mode: bool,
        pathfinding_points: &[Vec3]
    ) {
        // A. Preload/update meshes and textures to avoid mutable borrow checker clashes
        // with the render pass immutable borrow of self.depth_view

        // Update meshes
        let mut mesh_updates = Vec::new();
        for entity in &scene.entities {
            if !entity.active {
                continue;
            }
            if let Some(mesh) = &entity.mesh {
                if !self.gpu_meshes.contains_key(&entity.id) || mesh.is_dirty.get() {
                    mesh_updates.push((entity.id, mesh.vertices.clone(), mesh.indices.clone()));
                    mesh.is_dirty.set(false);
                }
            }
        }

        for (id, vertices, indices) in mesh_updates {
            self.update_gpu_mesh(id, &vertices, &indices);
        }

        // Preload textures
        let mut tex_paths = Vec::new();
        for entity in &scene.entities {
            if !entity.active {
                continue;
            }
            if let Some(t_comp) = &entity.texture {
                tex_paths.push(t_comp.path.clone());
            }
        }

        for path in tex_paths {
            self.load_texture(&path);
        }

        // Update skybox texture if path changed
        if !scene.skybox_path.is_empty() && self.skybox_path != scene.skybox_path {
            let path = scene.skybox_path.clone();
            self.skybox_path = path.clone();
            self.skybox_texture = Some(self.load_texture(&path));
        } else if scene.skybox_path.is_empty() {
            self.skybox_path = "".to_string();
            self.skybox_texture = None;
        }

        // 1. Write Camera Matrix Uniform
        let aspect = self.size.width as f32 / self.size.height as f32;
        let view_proj = camera.build_view_projection(aspect);

        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array(),
            camera_pos: camera.position.to_array(),
            _pad: 0.0,
        };
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        // 2. Build and Write Lighting Uniforms
        let mut lighting_uniform = LightingUniform {
            ambient: AmbientLightUniform {
                color: scene.ambient_color.to_array(),
                intensity: scene.ambient_intensity,
            },
            dir_light: DirectionalLightUniform {
                direction: [0.0, -1.0, 0.0],
                _pad1: 0.0,
                color: [1.0, 1.0, 1.0],
                intensity: 0.0,
                _pad2: [0.0; 4],
            },
            point_lights: [PointLightUniform {
                position: [0.0, 0.0, 0.0],
                _pad1: 0.0,
                color: [0.0, 0.0, 0.0],
                intensity: 0.0,
                range: 0.0,
                _pad2: [0.0; 3],
            }; 4],
            spot_light: SpotlightUniform {
                position: [0.0, 0.0, 0.0],
                _pad1: 0.0,
                direction: [0.0, 0.0, 0.0],
                _pad2: 0.0,
                color: [0.0, 0.0, 0.0],
                intensity: 0.0,
                range: 0.0,
                inner_cone: 0.0,
                outer_cone: 0.0,
                _pad3: 0.0,
            },
            num_point_lights: 0,
            ssr_active: 0.0,
            ssr_quality: 0.0,
            ssr_temporal_upsampling: 0.0,
        };

        // Populate dynamic lights from the scene
        let mut pt_idx = 0;
        for entity in &scene.entities {
            if !entity.active { continue; }

            if let Some(light) = &entity.light {
                match light.light_type {
                    LightType::Ambient => {
                        lighting_uniform.ambient = AmbientLightUniform {
                            color: light.color.to_array(),
                            intensity: light.intensity,
                        };
                    }
                    LightType::Directional => {
                        let dir = entity.transform.rotation * Vec3::NEG_Z;
                        lighting_uniform.dir_light = DirectionalLightUniform {
                            direction: dir.to_array(),
                            _pad1: 0.0,
                            color: light.color.to_array(),
                            intensity: light.intensity,
                            _pad2: [0.0; 4],
                        };
                    }
                    LightType::Point => {
                        if pt_idx < 4 {
                            lighting_uniform.point_lights[pt_idx] = PointLightUniform {
                                position: entity.transform.position.to_array(),
                                _pad1: 0.0,
                                color: light.color.to_array(),
                                intensity: light.intensity,
                                range: light.range,
                                _pad2: [0.0; 3],
                            };
                            pt_idx += 1;
                        }
                    }
                    LightType::Spotlight => {
                        let dir = entity.transform.rotation * Vec3::NEG_Z;
                        lighting_uniform.spot_light = SpotlightUniform {
                            position: entity.transform.position.to_array(),
                            _pad1: 0.0,
                            direction: dir.to_array(),
                            _pad2: 0.0,
                            color: light.color.to_array(),
                            intensity: light.intensity,
                            range: light.range,
                            inner_cone: light.inner_cone.to_radians().cos(),
                            outer_cone: light.outer_cone.to_radians().cos(),
                            _pad3: 0.0,
                        };
                    }
                }
            }
        }
        lighting_uniform.num_point_lights = pt_idx as u32;

        // Scan scene for active Visual Correction components (SSR)
        let mut ssr_active = 0.0;
        let mut ssr_quality = 2.0; // High default
        let mut ssr_temporal = 0.0;

        for entity in &scene.entities {
            if !entity.active { continue; }
            if let Some(vc) = &entity.visual_correction {
                if vc.ssr_active {
                    ssr_active = 1.0;
                }
                ssr_quality = match vc.ssr_quality.as_str() {
                    "Low" => 0.0,
                    "Medium" => 1.0,
                    "High" => 2.0,
                    "Ultra" => 3.0,
                    _ => 2.0,
                };
                if vc.ssr_temporal_upsampling {
                    ssr_temporal = 1.0;
                }
            }
        }

        lighting_uniform.ssr_active = ssr_active;
        lighting_uniform.ssr_quality = ssr_quality;
        lighting_uniform.ssr_temporal_upsampling = ssr_temporal;

        self.queue.write_buffer(&self.lighting_buffer, 0, bytemuck::bytes_of(&lighting_uniform));

        // 3. Pre-create all uniform buffers and bind groups to extend lifetimes
        // so that they outlive the render pass and prevent lifetime borrow-checker errors.
        let default_bones = BoneUniform {
            bones: [Mat4::IDENTITY.to_cols_array(); 64],
        };

        let mut solid_render_resources = Vec::new();
        for entity in &scene.entities {
            if !entity.active {
                continue;
            }

            if let Some(mesh) = &entity.mesh {
                if let Some(gpu_mesh) = self.gpu_meshes.get(&entity.id) {
                    // Prepare entity uniform buffer
                    let is_lit = if entity.light.is_some() { 0u32 } else { 1u32 };
                    let model_matrix = scene.compute_world_matrix(entity.id);
                    
                    let color_tint = if let Some(health) = &entity.health {
                        if health.is_dead {
                            [0.2, 0.2, 0.2, 1.0]
                        } else {
                            [1.0, 1.0, 1.0, 1.0]
                        }
                    } else if entity.name.starts_with("Enemy") {
                        [1.0, 0.3, 0.3, 1.0]
                    } else if entity.name == "Player" {
                        [0.3, 0.6, 1.0, 1.0]
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    };

                    let (metallic, roughness) = if let Some(t_comp) = &entity.texture {
                        (t_comp.metallic, t_comp.roughness)
                    } else {
                        (0.0, 0.5)
                    };

                    let entity_uniform = EntityUniform {
                        model_matrix: model_matrix.to_cols_array(),
                        color_tint,
                        use_texture: if entity.texture.is_some() { 1 } else { 0 },
                        is_lit,
                        metallic,
                        roughness,
                    };

                    let entity_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Entity Uniform"),
                        contents: bytemuck::bytes_of(&entity_uniform),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                    // Set bones buffer
                    let mut bones_data = default_bones;
                    if let Some(anim) = &entity.animator {
                        if anim.is_playing && !anim.freeze {
                            let wave = (anim.time * anim.speed).sin() * 0.15;
                            let joint_rot = Quat::from_rotation_z(wave);
                            let joint_matrix = Mat4::from_scale_rotation_translation(Vec3::ONE, joint_rot, Vec3::ZERO);
                            for i in 1..4 {
                                bones_data.bones[i] = joint_matrix.to_cols_array();
                            }
                        }
                    }

                    let bones_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Bones Uniform"),
                        contents: bytemuck::bytes_of(&bones_data),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                    // Bind Group 1
                    let entity_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Entity Bind Group"),
                        layout: &self.entity_bones_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: entity_buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: bones_buffer.as_entire_binding(),
                            },
                        ],
                    });

                    // Bind Group 2 (Texture)
                    let tex = if let Some(t_comp) = &entity.texture {
                        self.gpu_textures.get(&t_comp.path).cloned().unwrap_or_else(|| Rc::clone(&self.default_texture))
                    } else {
                        Rc::clone(&self.default_texture)
                    };

                    solid_render_resources.push((
                        entity.id,
                        entity_buffer,
                        bones_buffer,
                        entity_bind_group,
                        tex,
                        gpu_mesh.num_indices,
                    ));
                }
            }
        }

        // Pre-create outline resources for selected entity silhouette (editor mode only)
        let mut outline_resources = None;
        if editor_mode {
            if let Some(selected_id) = scene.selected_entity_id {
                if let Some(entity) = scene.entities.iter().find(|e| e.id == selected_id) {
                    if entity.active && entity.mesh.is_some() {
                        if let Some(gpu_mesh) = self.gpu_meshes.get(&selected_id) {
                            // Scale up the model matrix slightly for the outline hull
                            let outline_scale = 1.05;
                            let scaled_transform = Mat4::from_scale_rotation_translation(
                                entity.transform.scale * outline_scale,
                                entity.transform.rotation,
                                entity.transform.position,
                            );

                            let outline_uniform = EntityUniform {
                                model_matrix: scaled_transform.to_cols_array(),
                                color_tint: [1.0, 0.5, 0.0, 1.0], // Vibrant glowing orange outline
                                use_texture: 0,
                                is_lit: 0,
                                metallic: 0.0,
                                roughness: 0.5,
                            };

                            let outline_ent_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Outline Entity Uniform"),
                                contents: bytemuck::bytes_of(&outline_uniform),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });
                            let outline_bones_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Outline Bones Uniform"),
                                contents: bytemuck::bytes_of(&default_bones),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });
                            let outline_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("Outline Bind Group"),
                                layout: &self.entity_bones_layout,
                                entries: &[
                                    wgpu::BindGroupEntry { binding: 0, resource: outline_ent_buf.as_entire_binding() },
                                    wgpu::BindGroupEntry { binding: 1, resource: outline_bones_buf.as_entire_binding() },
                                ],
                            });

                            outline_resources = Some((
                                selected_id,
                                outline_ent_buf,
                                outline_bones_buf,
                                outline_bind_group,
                                gpu_mesh.num_indices,
                            ));
                        }
                    }
                }
            }
        }

        // Pre-create Grid bind group
        let mut grid_resources = None;
        if editor_mode {
            if let Some(grid_buf) = &self.grid_vertex_buffer {
                let grid_uniform = EntityUniform {
                    model_matrix: Mat4::IDENTITY.to_cols_array(),
                    color_tint: [0.15, 0.15, 0.22, 1.0], // Neon slate blue grid
                    use_texture: 0,
                    is_lit: 0,
                    metallic: 0.0,
                    roughness: 0.5,
                };
                let grid_buf_unif = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Grid Unif"),
                    contents: bytemuck::bytes_of(&grid_uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let default_bones_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Grid Bones Unif"),
                    contents: bytemuck::bytes_of(&default_bones),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let grid_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.entity_bones_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: grid_buf_unif.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: default_bones_buf.as_entire_binding() },
                    ],
                });
                grid_resources = Some((grid_buf_unif, default_bones_buf, grid_bind_group));
            }
        }

        // Pre-create AABB wireframes
        let mut aabb_resources = Vec::new();
        if editor_mode {
            for entity in &scene.entities {
                if !entity.active { continue; }
                if scene.selected_entity_id == Some(entity.id) { continue; }
                if let Some(col) = &entity.collider {
                    if !col.active { continue; }

                    let min = col.aabb_min;
                    let max = col.aabb_max;

                    // Generate wireframe box lines (12 lines = 24 vertices)
                    let mut line_vertices = Vec::new();
                    let normal = Vec3::Y;
                    let uv = [0.0, 0.0];

                    let p000 = Vec3::new(min.x, min.y, min.z);
                    let p100 = Vec3::new(max.x, min.y, min.z);
                    let p010 = Vec3::new(min.x, max.y, min.z);
                    let p110 = Vec3::new(max.x, max.y, min.z);
                    let p001 = Vec3::new(min.x, min.y, max.z);
                    let p101 = Vec3::new(max.x, min.y, max.z);
                    let p011 = Vec3::new(min.x, max.y, max.z);
                    let p111 = Vec3::new(max.x, max.y, max.z);

                    // Bottom face
                    line_vertices.push(Vertex::new(p000, normal, uv)); line_vertices.push(Vertex::new(p100, normal, uv));
                    line_vertices.push(Vertex::new(p100, normal, uv)); line_vertices.push(Vertex::new(p101, normal, uv));
                    line_vertices.push(Vertex::new(p101, normal, uv)); line_vertices.push(Vertex::new(p001, normal, uv));
                    line_vertices.push(Vertex::new(p001, normal, uv)); line_vertices.push(Vertex::new(p000, normal, uv));

                    // Top face
                    line_vertices.push(Vertex::new(p010, normal, uv)); line_vertices.push(Vertex::new(p110, normal, uv));
                    line_vertices.push(Vertex::new(p110, normal, uv)); line_vertices.push(Vertex::new(p111, normal, uv));
                    line_vertices.push(Vertex::new(p111, normal, uv)); line_vertices.push(Vertex::new(p011, normal, uv));
                    line_vertices.push(Vertex::new(p011, normal, uv)); line_vertices.push(Vertex::new(p010, normal, uv));

                    // Verticals connecting bottom and top
                    line_vertices.push(Vertex::new(p000, normal, uv)); line_vertices.push(Vertex::new(p010, normal, uv));
                    line_vertices.push(Vertex::new(p100, normal, uv)); line_vertices.push(Vertex::new(p110, normal, uv));
                    line_vertices.push(Vertex::new(p101, normal, uv)); line_vertices.push(Vertex::new(p111, normal, uv));
                    line_vertices.push(Vertex::new(p001, normal, uv)); line_vertices.push(Vertex::new(p011, normal, uv));

                    let aabb_wire_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&line_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

                    // Bright glowing green if selected, cyan otherwise
                    let is_selected = scene.selected_entity_id == Some(entity.id);
                    let tint_color = if is_selected { [0.0, 1.0, 0.4, 1.0] } else { [0.0, 0.8, 1.0, 0.8] };

                    let entity_uniform = EntityUniform {
                        model_matrix: Mat4::IDENTITY.to_cols_array(), // Vertices are already in world space
                        color_tint: tint_color,
                        use_texture: 0,
                        is_lit: 0,
                        metallic: 0.0,
                        roughness: 0.5,
                    };

                    let entity_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::bytes_of(&entity_uniform),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                    let default_bones_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::bytes_of(&default_bones),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                    let col_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.entity_bones_layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: entity_buf.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: default_bones_buf.as_entire_binding() },
                        ],
                    });

                    aabb_resources.push((aabb_wire_buffer, entity_buf, default_bones_buf, col_bind_group));
                }
            }
        }

        // Pre-create axis arrows for the selected entity in EditorMode
        let mut axis_arrow_resources = Vec::new();
        if editor_mode {
            if let Some(selected_id) = scene.selected_entity_id {
                if let Some(_entity) = scene.entities.iter().find(|e| e.id == selected_id) {
                    let world_matrix = scene.compute_world_matrix(selected_id);
                    let world_pos = world_matrix.col(3).truncate();
                    let arrow_model_matrix = Mat4::from_translation(world_pos);

                    let colors = [
                        [1.0, 0.1, 0.1, 1.0], // X: Red
                        [0.1, 0.9, 0.1, 1.0], // Y: Green
                        [0.1, 0.4, 1.0, 1.0], // Z: Blue
                    ];

                    for (i, color) in colors.iter().enumerate() {
                        let entity_uniform = EntityUniform {
                            model_matrix: arrow_model_matrix.to_cols_array(),
                            color_tint: *color,
                            use_texture: 0,
                            is_lit: 0,
                            metallic: 0.0,
                            roughness: 0.5,
                        };

                        let entity_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Axis Arrow Uniform"),
                            contents: bytemuck::bytes_of(&entity_uniform),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });

                        let default_bones_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Axis Arrow Bones"),
                            contents: bytemuck::bytes_of(&default_bones),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });

                        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("Axis Arrow Bind Group"),
                            layout: &self.entity_bones_layout,
                            entries: &[
                                wgpu::BindGroupEntry { binding: 0, resource: entity_buf.as_entire_binding() },
                                wgpu::BindGroupEntry { binding: 1, resource: default_bones_buf.as_entire_binding() },
                            ],
                        });

                        axis_arrow_resources.push((i, entity_buf, default_bones_buf, bind_group));
                    }
                }
            }
        }

        // Pre-create Path vertices
        let mut path_resources = None;
        if pathfinding_points.len() >= 2 {
            let mut path_vertices = Vec::new();
            let normal = Vec3::Y;
            let uv = [0.0, 0.0];

            for i in 0..pathfinding_points.len() - 1 {
                // Lift points slightly above floor grid to prevent z-fighting
                let p1 = pathfinding_points[i] + Vec3::new(0.0, 0.05, 0.0);
                let p2 = pathfinding_points[i+1] + Vec3::new(0.0, 0.05, 0.0);

                path_vertices.push(Vertex::new(p1, normal, uv));
                path_vertices.push(Vertex::new(p2, normal, uv));
            }

            let path_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Path Vertices"),
                contents: bytemuck::cast_slice(&path_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let entity_uniform = EntityUniform {
                model_matrix: Mat4::IDENTITY.to_cols_array(),
                color_tint: [0.0, 1.0, 0.3, 1.0], // Neon green pathline
                use_texture: 0,
                is_lit: 0,
                metallic: 0.0,
                roughness: 0.5,
            };

            let entity_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&entity_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let default_bones_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&default_bones),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let path_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.entity_bones_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: entity_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: default_bones_buf.as_entire_binding() },
                ],
            });

            path_resources = Some((path_buffer, entity_buf, default_bones_buf, path_bind_group, path_vertices.len() as u32));
        }

        // 4. Render Pass Setup
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Scene Render Encoder"),
        });

        // A. Shadow depth sweep passes
        let mut dir_light_dir = Vec3::new(-0.5, -1.0, -0.3).normalize();
        for entity in &scene.entities {
            if entity.active {
                if let Some(light) = &entity.light {
                    if light.light_type == LightType::Directional {
                        dir_light_dir = (entity.transform.rotation * Vec3::NEG_Z).normalize();
                    }
                }
            }
        }
        self.shadow_renderer.update_light_space(&self.queue, dir_light_dir);
        self.queue.write_buffer(
            &self.shadow_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.shadow_renderer.light_space_matrix.to_cols_array()),
        );

        if !self.shadow_renderer.is_static_cached {
            self.shadow_renderer.render_static(&self.device, &mut encoder, scene, &self.gpu_meshes);
        }
        self.shadow_renderer.render_dynamic(&self.device, &mut encoder, scene, &self.gpu_meshes);

        // Dynamic global bind group updates to bind active skybox view & sampler for reflections
        let skybox_view = self.skybox_texture.as_ref()
            .map(|tex| &tex.view)
            .unwrap_or(&self.default_texture.view);
            
        let skybox_sampler = self.skybox_texture.as_ref()
            .map(|tex| &tex.sampler)
            .unwrap_or(&self.default_texture.sampler);
            
        self.global_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group with Reflections"),
            layout: &self.camera_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(skybox_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(skybox_sampler),
                },
            ],
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_texture,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06, // Sleek modern dark backdrop
                            g: 0.06,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set global bindings
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);
            render_pass.set_bind_group(3, &self.shadow_bind_group, &[]);

            // Render Solid Entities
            for (id, _ent_buf, _bones_buf, bind_group, tex, num_indices) in &solid_render_resources {
                if let Some(gpu_mesh) = self.gpu_meshes.get(id) {
                    render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.set_bind_group(1, bind_group, &[]);
                    render_pass.set_bind_group(2, &tex.bind_group, &[]);
                    render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                }
            }

            // Render Selection Outline Silhouette (if in editor mode)
            if editor_mode {
                if let Some((selected_id, _outline_ent_buf, _outline_bones_buf, outline_bind_group, num_indices)) = &outline_resources {
                    if let Some(gpu_mesh) = self.gpu_meshes.get(selected_id) {
                        let tex = solid_render_resources.iter()
                            .find(|(id, _, _, _, _, _)| id == selected_id)
                            .map(|(_, _, _, _, tex, _)| tex)
                            .unwrap_or(&self.default_texture);

                        render_pass.set_pipeline(&self.outline_pipeline);
                        render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        render_pass.set_bind_group(1, outline_bind_group, &[]);
                        render_pass.set_bind_group(2, &tex.bind_group, &[]);
                        render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                    }
                }
            }

            // Render Skybox last (optimization)
            if let Some(skybox_tex) = &self.skybox_texture {
                self.skybox_renderer.draw(&mut render_pass, &self.global_bind_group, skybox_tex);
            }

            // 5. Render debug overlay tools
            render_pass.set_pipeline(&self.line_pipeline);

            // A. Draw floor grid (in EditorMode only)
            if editor_mode {
                if let Some((_grid_buf_unif, _default_bones_buf, grid_bind_group)) = &grid_resources {
                    if let Some(grid_buf) = &self.grid_vertex_buffer {
                        render_pass.set_vertex_buffer(0, grid_buf.slice(..));
                        render_pass.set_bind_group(1, grid_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.default_texture.bind_group, &[]);
                        render_pass.draw(0..self.grid_count, 0..1);
                    }
                }

                // B. Draw AABB outlines for active colliders
                for (aabb_wire_buffer, _entity_buf, _default_bones_buf, col_bind_group) in &aabb_resources {
                    render_pass.set_vertex_buffer(0, aabb_wire_buffer.slice(..));
                    render_pass.set_bind_group(1, col_bind_group, &[]);
                    render_pass.draw(0..24, 0..1);
                }

                // C. Draw global axis arrows overlay for the selected entity
                for (i, _entity_buf, _default_bones_buf, bind_group) in &axis_arrow_resources {
                    let buffer = match i {
                        0 => &self.axis_x_buffer,
                        1 => &self.axis_y_buffer,
                        2 => &self.axis_z_buffer,
                        _ => &None,
                    };
                    if let Some(buf) = buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, bind_group, &[]);
                        render_pass.draw(0..self.axis_count, 0..1);
                    }
                }
            }


        } // End of Render Pass

        // 6. Submit Render commands
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
