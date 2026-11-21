use crate::gpu::layout_entries::uniform_entry;
use crate::gpu::ops::CountedDevice as _;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::DEPTH_FORMAT;
use crate::postfx::effects::taa;
use crate::postfx::HDR_FORMAT;
use rusty_components::ParticleBlend;

pub(crate) struct Globals {
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl Globals {
    pub(crate) fn entries() -> [wgpu::BindGroupLayoutEntry; 1] {
        [uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)]
    }

    pub(crate) fn new(device: &wgpu::Device, name: &str, size: u64) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{name} Globals Layout")),
            entries: &Self::entries(),
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{name} Globals")),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
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

pub(crate) struct BlendPipelines {
    alpha: LazyPipeline,
    additive: LazyPipeline,
    alpha_reactive: LazyPipeline,
    additive_reactive: LazyPipeline,
}

impl BlendPipelines {
    pub(crate) fn new(
        device: &wgpu::Device,
        label: &'static str,
        (shader, layout): (&wgpu::ShaderModule, &wgpu::PipelineLayout),
        (entry, vertex): (&'static str, Option<wgpu::VertexBufferLayout<'static>>),
    ) -> Self {
        let lazy = |blend, fragment_entry: &'static str, reactive: bool| {
            let (shader, layout, vertex) = (shader.clone(), layout.clone(), vertex.clone());
            LazyPipeline::new(device, move |device| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        buffers: &[vertex.clone()][..usize::from(vertex.is_some())],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(fragment_entry),
                        compilation_options: Default::default(),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: HDR_FORMAT,
                                blend: Some(blend),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(taa::reactive::target_state()),
                        ][..1 + usize::from(reactive)],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        // Overlapping translucents must all blend, not z-fight.
                        depth_write_enabled: Some(false),
                        depth_compare: Some(wgpu::CompareFunction::LessEqual),
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
            })
        };
        let alpha = wgpu::BlendState::ALPHA_BLENDING;
        Self {
            alpha: lazy(alpha, "fs_alpha", false),
            additive: lazy(additive_blend(), "fs_additive", false),
            alpha_reactive: lazy(alpha, "fs_alpha_reactive", true),
            additive_reactive: lazy(additive_blend(), "fs_additive_reactive", true),
        }
    }

    pub(crate) fn pipeline_for(
        &self,
        blend: ParticleBlend,
        reactive: bool,
    ) -> &wgpu::RenderPipeline {
        match (blend, reactive) {
            (ParticleBlend::Alpha, false) => self.alpha.get(),
            (ParticleBlend::Additive, false) => self.additive.get(),
            (ParticleBlend::Alpha, true) => self.alpha_reactive.get(),
            (ParticleBlend::Additive, true) => self.additive_reactive.get(),
        }
    }

    pub(crate) fn lazy(&self) -> [&LazyPipeline; 4] {
        [
            &self.alpha,
            &self.additive,
            &self.alpha_reactive,
            &self.additive_reactive,
        ]
    }
}

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
