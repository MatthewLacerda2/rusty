//! src/render/passes/blended.rs — the shape the particle and ribbon passes share
//! (#939): a globals uniform at group 0, and an alpha and an additive pipeline into
//! the HDR target that test the scene depth without writing it.

use crate::components::ParticleBlend;
use crate::render::gpu::layout_entries::uniform_entry;
use crate::render::gpu::DEPTH_FORMAT;
use crate::render::postfx::HDR_FORMAT;

/// A pass's group-0 globals: one `size`-byte uniform, its layout and bind group,
/// labelled `"{name} Globals …"`.
pub(crate) struct Globals {
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl Globals {
    pub(crate) fn new(device: &wgpu::Device, name: &str, size: u64) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{name} Globals Layout")),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{name} Globals")),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{name} Globals Bind Group")),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            layout,
            buffer,
            bind_group,
        }
    }
}

/// The alpha and additive pipelines of one blended pass.
pub(crate) struct BlendPipelines {
    alpha: wgpu::RenderPipeline,
    additive: wgpu::RenderPipeline,
}

impl BlendPipelines {
    /// Build both from `shader`'s `vs_main` and its `fs_alpha` / `fs_additive`,
    /// drawing `vertex` buffers into the HDR target. Both faces are drawn, and the
    /// scene depth is tested but never written: overlapping translucent sprites and
    /// strips must all blend, not z-fight, and the same depth can also be bound as a
    /// texture (soft particles).
    pub(crate) fn new(
        device: &wgpu::Device,
        label: &str,
        (shader, layout): (&wgpu::ShaderModule, &wgpu::PipelineLayout),
        vertex: wgpu::VertexBufferLayout<'_>,
    ) -> Self {
        let pipeline = |blend, fragment_entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(vertex.clone())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            alpha: pipeline(wgpu::BlendState::ALPHA_BLENDING, "fs_alpha"),
            additive: pipeline(additive_blend(), "fs_additive"),
        }
    }

    pub(crate) fn pipeline_for(&self, blend: ParticleBlend) -> &wgpu::RenderPipeline {
        match blend {
            ParticleBlend::Alpha => &self.alpha,
            ParticleBlend::Additive => &self.additive,
        }
    }
}

/// Additive blend (src + dst), keeping dst alpha. Sparks/fire brighten the HDR
/// target so the bloom pass picks them up.
pub(crate) fn additive_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    }
}
