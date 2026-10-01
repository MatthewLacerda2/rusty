//! Construction of the shadow pass's GPU resources: the static + active cascade depth
//! arrays, the comparison sampler the forward shader reads them through, and the
//! depth-only pipelines. Split out of `shadows/mod.rs` so each file stays under the size
//! cap.

use super::cascades::MAX_CASCADES;
use super::ShadowRenderer;
use crate::render::gpu::bind_layouts::storage_entry;
use crate::render::gpu::mesh::vertex_layout;

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
    /// light-space binding takes a dynamic offset: one matrix per cascade; the time
    /// buffer is a `CameraUniform` whose `time` alone is written. Binding numbers are
    /// the ones `shader.wgsl`'s shadow stage declares beside the forward pass's (#648).
    pub(super) fn create_pass_layouts(
        device: &wgpu::Device,
        [light_space_buffer, time_buffer]: [&wgpu::Buffer; 2],
    ) -> (
        wgpu::BindGroupLayout,
        wgpu::BindGroup,
        wgpu::BindGroupLayout,
    ) {
        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Global Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: LIGHT_BINDING,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                },
                // The forward `camera` slot, holding only the game time: a cutting
                // variant's UV chain reads `camera.time` (#648).
                wgpu::BindGroupLayoutEntry {
                    binding: TIME_BINDING,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Global Bind Group"),
            layout: &global_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: LIGHT_BINDING,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: light_space_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: TIME_BINDING,
                    resource: time_buffer.as_entire_binding(),
                },
            ],
        });

        // Every caster as one storage array, indexed per instance (#470) — and read by
        // the clip's fragment for its alpha test (#648) — and the joint matrices its
        // skinned casters are posed with (#599).
        let entity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Caster Layout"),
            entries: &[
                storage_entry(
                    CASTERS_BINDING,
                    wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ),
                storage_entry(JOINTS_BINDING, wgpu::ShaderStages::VERTEX),
            ],
        });

        (global_layout, global_bind_group, entity_layout)
    }

    /// The plain depth-only pipeline and the clipping one (#648), both from the
    /// forward `shader`'s shadow stage; the clip pipeline's layout adds the forward
    /// pass's group-2 material layout to the two shadow groups.
    pub(super) fn create_pipelines(
        device: &wgpu::Device,
        [global, entity, material]: [&wgpu::BindGroupLayout; 3],
        shader: &wgpu::ShaderModule,
    ) -> [wgpu::RenderPipeline; 2] {
        let plain = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[global, entity],
            push_constant_ranges: &[],
        });
        let clip = clip_layout(device, [global, entity, material]);
        [
            depth_pipeline(device, shader, &plain, None),
            depth_pipeline(device, shader, &clip, Some("fs_shadow")),
        ]
    }
}

/// Where `shader.wgsl`'s shadow stage reads the cascade's matrix and the game time
/// (group 0), its casters and their joints (group 1) — beside the forward pass's
/// bindings, sharing `camera` and `bones` (#648).
const LIGHT_BINDING: u32 = 6;
const TIME_BINDING: u32 = 0;
pub(super) const CASTERS_BINDING: u32 = 3;
pub(super) const JOINTS_BINDING: u32 = 1;

/// The clipping shadow pipelines' layout: light, casters, material (#648).
pub(crate) fn clip_layout(
    device: &wgpu::Device,
    layouts: [&wgpu::BindGroupLayout; 3],
) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Shadow Clip Pipeline Layout"),
        bind_group_layouts: &layouts,
        push_constant_ranges: &[],
    })
}

/// A shadow-cascade depth pipeline over `shader`'s `vs_shadow`: depth only with no
/// `fragment`, or clipped by one (`fs_shadow`, a cutting variant's `fs_shadow_cut`,
/// #648). Front faces culled and a sloped bias, to fight shadow acne.
pub(crate) fn depth_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: Option<&str>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(match fragment {
            None => "Shadow Render Pipeline",
            Some(_) => "Shadow Clip Pipeline",
        }),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_shadow",
            buffers: &[vertex_layout()],
        },
        fragment: fragment.map(|entry_point| wgpu::FragmentState {
            module: shader,
            entry_point,
            targets: &[],
        }),
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
