//! src/render/gpu/fullscreen.rs — the pipeline every fullscreen-triangle pass builds
//! (#939): no vertex buffers (the vertex stage makes the triangle from its index),
//! default primitive state, no multisampling. Post-FX, SSAO, the UI backdrop blur
//! and the shadow-atlas tile blit differ only in their entries, targets and depth.
//! Also [`color_pass`], the timed colour-only pass the UI and the selection outline
//! record into.

use crate::render::timing::{GpuPass, GpuTimer};

/// One fullscreen pass's shader, layout and entry points.
pub(crate) struct Fullscreen<'a> {
    pub(crate) label: &'a str,
    pub(crate) shader: &'a wgpu::ShaderModule,
    pub(crate) layout: &'a wgpu::PipelineLayout,
    pub(crate) vs: &'a str,
    pub(crate) fs: &'a str,
}

impl Fullscreen<'_> {
    /// A pipeline writing `targets`, with no depth attachment.
    pub(crate) fn colour(
        &self,
        device: &wgpu::Device,
        targets: &[Option<wgpu::ColorTargetState>],
    ) -> wgpu::RenderPipeline {
        self.build(device, targets, None)
    }

    /// A pipeline writing only depth (no colour targets), through `depth`.
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

/// A colour-only render pass over `target`, loading or clearing it, timed as `pass`
/// (#835): the UI's overlay, mask and blur passes, the selection outline's.
pub(crate) fn color_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    (label, timer, pass): (&str, &GpuTimer, GpuPass),
    target: &'e wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
