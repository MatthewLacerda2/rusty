use crate::gpu::ops::CountedEncoder as _;
use crate::timing::{GpuPass, GpuTimer};

pub(crate) struct Fullscreen<'a> {
    pub(crate) label: &'a str,
    pub(crate) shader: &'a wgpu::ShaderModule,
    pub(crate) layout: &'a wgpu::PipelineLayout,
    pub(crate) vs: &'a str,
    pub(crate) fs: &'a str,
}

impl Fullscreen<'_> {
    pub(crate) fn colour(
        &self,
        device: &wgpu::Device,
        targets: &[Option<wgpu::ColorTargetState>],
    ) -> wgpu::RenderPipeline {
        self.build(device, targets, None)
    }

    pub(crate) fn depth_only(
        &self,
        device: &wgpu::Device,
        depth: wgpu::DepthStencilState,
    ) -> wgpu::RenderPipeline {
        self.build(device, &[], Some(depth))
    }

    fn build(
        &self,
        device: &wgpu::Device,
        targets: &[Option<wgpu::ColorTargetState>],
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(self.label),
            layout: Some(self.layout),
            vertex: wgpu::VertexState {
                module: self.shader,
                entry_point: Some(self.vs),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: self.shader,
                entry_point: Some(self.fs),
                compilation_options: Default::default(),
                targets,
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}

pub(crate) fn color_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    (label, timer, pass): (&str, &GpuTimer, GpuPass),
    target: &'e wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: timer.writes(pass),
        occlusion_query_set: None,
    })
}
