use crate::core::scene::Scene;
use crate::render::mesh::Vertex;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub struct ShadowRenderer {
    pub static_texture: wgpu::Texture,
    pub static_view: wgpu::TextureView,
    pub active_texture: wgpu::Texture,
    pub active_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,

    pipeline: wgpu::RenderPipeline,
    light_space_buffer: wgpu::Buffer,
    pub light_space_matrix: Mat4,

    pub is_static_cached: bool,

    global_bind_group: wgpu::BindGroup,
    entity_layout: wgpu::BindGroupLayout,
}

impl ShadowRenderer {
    pub const SHADOW_SIZE: u32 = 2048;

    pub fn new(device: &wgpu::Device) -> Self {
        // Create depth textures
        let size = wgpu::Extent3d {
            width: Self::SHADOW_SIZE,
            height: Self::SHADOW_SIZE,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("Shadow Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let static_texture = device.create_texture(&desc);
        let static_view = static_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let active_texture = device.create_texture(&desc);
        let active_view = active_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Sampler with comparison for hardware PCF shadows
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // Bind group layout to expose shadow depth map to main shader
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Map Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Map Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&active_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Compile shadow map shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/shadow.wgsl").into(),
            ),
        });

        let light_space_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Light Space Buffer"),
            size: 64, // Mat4 size
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Global Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Global Bind Group"),
            layout: &global_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_space_buffer.as_entire_binding(),
            }],
        });

        let entity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Entity Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[&global_layout, &entity_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: None, // Depth only pass
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2, // Sloped depth bias to prevent shadow acne
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            static_texture,
            static_view,
            active_texture,
            active_view,
            sampler,
            bind_group_layout,
            bind_group,
            pipeline,
            light_space_buffer,
            light_space_matrix: Mat4::IDENTITY,
            is_static_cached: false,
            global_bind_group,
            entity_layout,
        }
    }

    pub fn update_light_space(&mut self, queue: &wgpu::Queue, light_dir: Vec3) {
        let norm_dir = light_dir.normalize();
        // Position the shadow camera looking at the center of the scene
        let center = Vec3::ZERO;
        let shadow_cam_pos = center - norm_dir * 45.0;
        let view = Mat4::look_at_rh(shadow_cam_pos, center, Vec3::Y);

        // Orthographic projection suitable for typical scenes
        let proj = Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 1.0, 100.0);
        self.light_space_matrix = proj * view;

        queue.write_buffer(
            &self.light_space_buffer,
            0,
            bytemuck::bytes_of(&self.light_space_matrix.to_cols_array()),
        );
    }

    pub fn render_static(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        gpu_meshes: &HashMap<u32, crate::render::GpuMesh>,
    ) {
        let mut render_resources = Vec::new();
        for entity in scene.iter() {
            if !entity.active || !entity.is_static {
                continue;
            }
            if entity.mesh.is_some() {
                if let Some(gpu_mesh) = gpu_meshes.get(&entity.id) {
                    let model_matrix = scene.compute_world_matrix(entity.id);
                    let model_arr = model_matrix.to_cols_array();
                    let entity_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Shadow Static Entity Uniform"),
                        contents: bytemuck::bytes_of(&model_arr),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Shadow Static Entity Bind Group"),
                        layout: &self.entity_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: entity_buf.as_entire_binding(),
                        }],
                    });

                    render_resources.push((entity.id, bind_group, gpu_mesh.num_indices));
                }
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Static Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.static_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);

            for (id, bind_group, num_indices) in &render_resources {
                if let Some(gpu_mesh) = gpu_meshes.get(id) {
                    render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        gpu_mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.set_bind_group(1, bind_group, &[]);
                    render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                }
            }
        }

        self.is_static_cached = true;
    }

    pub fn render_dynamic(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        gpu_meshes: &HashMap<u32, crate::render::GpuMesh>,
    ) {
        let size = wgpu::Extent3d {
            width: Self::SHADOW_SIZE,
            height: Self::SHADOW_SIZE,
            depth_or_array_layers: 1,
        };

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.static_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &self.active_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            size,
        );

        let mut render_resources = Vec::new();
        for entity in scene.iter() {
            if !entity.active || entity.is_static {
                continue;
            }
            if entity.mesh.is_some() {
                if let Some(gpu_mesh) = gpu_meshes.get(&entity.id) {
                    let model_matrix = scene.compute_world_matrix(entity.id);
                    let model_arr = model_matrix.to_cols_array();
                    let entity_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Shadow Dynamic Entity Uniform"),
                        contents: bytemuck::bytes_of(&model_arr),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Shadow Dynamic Entity Bind Group"),
                        layout: &self.entity_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: entity_buf.as_entire_binding(),
                        }],
                    });

                    render_resources.push((entity.id, bind_group, gpu_mesh.num_indices));
                }
            }
        }

        if !render_resources.is_empty() {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Dynamic Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.active_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);

            for (id, bind_group, num_indices) in &render_resources {
                if let Some(gpu_mesh) = gpu_meshes.get(id) {
                    render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        gpu_mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.set_bind_group(1, bind_group, &[]);
                    render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                }
            }
        }
    }
}
