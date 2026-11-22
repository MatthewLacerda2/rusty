pub(crate) mod draw;
pub(crate) mod strip;

use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::uniforms::FogUniform;
use crate::passes::blended::{BlendPipelines, Globals};
use crate::{GpuTexture, Renderer};
use rusty_components::ParticleBlend;
use strip::RibbonVertex;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RibbonGlobals {
    pub(crate) view_proj: [f32; 16],
    pub(crate) cam_pos: [f32; 4],
    pub(crate) fog: FogUniform,
}

pub struct RibbonRenderer {
    pipelines: BlendPipelines,
    pub(crate) globals_buffer: wgpu::Buffer,
    pub(crate) globals_bind_group: wgpu::BindGroup,
    pub(crate) white: GpuTexture,
}

impl RibbonRenderer {
    pub(crate) fn new(
        (device, writes): (&wgpu::Device, crate::gpu::ops::Writes),
        texture_layout: &wgpu::BindGroupLayout,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let shader = registry.load(device, "ribbons.wgsl", "Ribbon Shader");
        let globals = Globals::new(
            device,
            "Ribbon",
            std::mem::size_of::<RibbonGlobals>() as u64,
        );
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ribbon Pipeline Layout"),
            bind_group_layouts: &[Some(&globals.layout), Some(texture_layout)],
            immediate_size: 0,
        });
        Self {
            pipelines: BlendPipelines::new(
                device,
                "Ribbon Pipeline",
                (&shader, &layout),
                ("vs_main", Some(RibbonVertex::desc())),
            ),
            globals_buffer: globals.buffer,
            globals_bind_group: globals.bind_group,
            white: Renderer::create_white_texture((device, writes), texture_layout),
        }
    }

    pub(crate) fn pipeline_for(
        &self,
        blend: ParticleBlend,
        reactive: bool,
    ) -> &wgpu::RenderPipeline {
        self.pipelines.pipeline_for(blend, reactive)
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 4] {
        self.pipelines.lazy()
    }
}
