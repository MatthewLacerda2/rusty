//! src/render/gpu/fullscreen.rs — the pipeline every fullscreen-triangle pass builds
//! (#939): no vertex buffers (the vertex stage makes the triangle from its index),
//! default primitive state, no multisampling. Post-FX, SSAO, the UI backdrop blur
//! and the shadow-atlas tile blit differ only in their entries, targets and depth.

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
