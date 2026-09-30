//! Construction of the shadow pass's GPU resources: the static + active depth maps, the
//! comparison sampler the forward shader reads them through, and the depth-only
//! pipeline. Split out of `shadows/mod.rs` so each file stays under the size cap.

use super::ShadowRenderer;
use crate::render::gpu::bind_layouts::storage_entry;
use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::shaders::ShaderRegistry;

impl ShadowRenderer {
    /// Allocate the static + active depth textures (and their default views).
    pub(super) fn create_depth_textures(
        device: &wgpu::Device,
    ) -> (
        wgpu::Texture,
        wgpu::TextureView,
        wgpu::Texture,
        wgpu::TextureView,
    ) {
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

        (static_texture, static_view, active_texture, active_view)
    }

    /// Comparison sampler plus the layout/group that expose the depth map to the
    /// main shader.
    pub(super) fn create_sampler_and_bind_group(
        device: &wgpu::Device,
        active_view: &wgpu::TextureView,
    ) -> (wgpu::Sampler, wgpu::BindGroupLayout, wgpu::BindGroup) {
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
                    resource: wgpu::BindingResource::TextureView(active_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        (sampler, bind_group_layout, bind_group)
    }

    /// Global (light-space) and per-entity layouts used by the depth-only pass.
    pub(super) fn create_pass_layouts(
        device: &wgpu::Device,
        light_space_buffer: &wgpu::Buffer,
    ) -> (
        wgpu::BindGroupLayout,
        wgpu::BindGroup,
        wgpu::BindGroupLayout,
    ) {
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

        // Every caster's world matrix as one storage array, indexed per instance (#470).
        let entity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Caster Layout"),
            entries: &[storage_entry(0, wgpu::ShaderStages::VERTEX)],
        });

        (global_layout, global_bind_group, entity_layout)
    }

    /// Depth-only render pipeline (sloped bias to fight shadow acne).
    pub(super) fn create_pipeline(
        device: &wgpu::Device,
        global_layout: &wgpu::BindGroupLayout,
        entity_layout: &wgpu::BindGroupLayout,
        registry: &mut ShaderRegistry,
    ) -> wgpu::RenderPipeline {
        let shader = registry.load(device, "shadow.wgsl", "Shadow Shader");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[global_layout, entity_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_layout()],
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
        })
    }
}
