//! Construction of the shadow pass's GPU resources: the static + active cascade depth
//! arrays, the comparison sampler the forward shader reads them through, and the
//! depth-only pipeline. Split out of `shadows/mod.rs` so each file stays under the size
//! cap.

use super::cascades::MAX_CASCADES;
use super::ShadowRenderer;
use crate::render::gpu::bind_layouts::storage_entry;
use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::shaders::ShaderRegistry;

/// The two cascade depth arrays and their views.
pub(super) struct DepthTextures {
    pub static_texture: wgpu::Texture,
    pub active_texture: wgpu::Texture,
    pub static_layers: Vec<wgpu::TextureView>,
    pub active_layers: Vec<wgpu::TextureView>,
    pub active_view: wgpu::TextureView,
}

impl ShadowRenderer {
    /// Allocate the static + active depth arrays, one layer per cascade, with a
    /// render-target view per layer and the active array's sampled view.
    pub(super) fn create_depth_textures(device: &wgpu::Device) -> DepthTextures {
        let desc = wgpu::TextureDescriptor {
            label: Some("Shadow Cascade Depth Array"),
            size: wgpu::Extent3d {
                width: Self::CASCADE_SIZE,
                height: Self::CASCADE_SIZE,
                depth_or_array_layers: MAX_CASCADES as u32,
            },
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
        let layers = |texture: &wgpu::Texture| -> Vec<wgpu::TextureView> {
            (0..MAX_CASCADES as u32)
                .map(|layer| {
                    texture.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_array_layer: layer,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect()
        };

        let static_texture = device.create_texture(&desc);
        let active_texture = device.create_texture(&desc);
        let active_view = active_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        DepthTextures {
            static_layers: layers(&static_texture),
            active_layers: layers(&active_texture),
            static_texture,
            active_texture,
            active_view,
        }
    }

    /// The comparison sampler the forward shader's PCF reads the cascades through.
    pub(super) fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        })
    }

    /// Global (light-space) and per-entity layouts used by the depth-only pass. The
    /// light-space binding takes a dynamic offset: one matrix per cascade.
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
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(64),
                },
                count: None,
            }],
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Global Bind Group"),
            layout: &global_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: light_space_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(64),
                }),
            }],
        });

        // Every caster as one storage array, indexed per instance (#470), and the joint
        // matrices its skinned casters are posed with (#599).
        let entity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Caster Layout"),
            entries: &[
                storage_entry(0, wgpu::ShaderStages::VERTEX),
                storage_entry(1, wgpu::ShaderStages::VERTEX),
            ],
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
