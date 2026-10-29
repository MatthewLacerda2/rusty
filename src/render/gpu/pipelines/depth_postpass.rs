//! The transparent depth postpass's pipelines (#1523), HDRP's *Transparent Depth
//! Postpass*: a translucent surface that asks for it draws again after the
//! translucents, writing only its depth (tested `LessEqual`, so the forward vertex
//! stage's invariant position lands on its own depth) and, in a view with a velocity
//! target, its texel there. Each is built on its first draw.

use super::{velocity_target, LazyPipeline};
use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::DEPTH_FORMAT;

/// One shader's depth postpass pipelines: depth alone, and depth with the velocity
/// target.
pub(crate) struct DepthPostpassPipelines {
    plain: LazyPipeline,
    velocity: LazyPipeline,
}

impl DepthPostpassPipelines {
    /// `shader`'s pair through the forward `layout`, not yet built.
    pub(super) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> Self {
        let make = |velocity: bool| {
            let (shader, layout) = (shader.clone(), layout.clone());
            LazyPipeline::new(device, move |device| {
                build(device, &shader, &layout, velocity)
            })
        };
        Self {
            plain: make(false),
            velocity: make(true),
        }
    }

    /// The one for a view that writes a velocity target, or not.
    pub(crate) fn get(&self, velocity: bool) -> &wgpu::RenderPipeline {
        match velocity {
            false => self.plain.get(),
            true => self.velocity.get(),
        }
    }

    /// Both, for warming.
    pub(crate) fn lazies(&self) -> [&LazyPipeline; 2] {
        [&self.plain, &self.velocity]
    }
}

/// The depth-only pipeline (`vs_main` / `fs_prepass`), or with `velocity` the one
/// writing the velocity target at location 1 (`vs_velocity` / `fs_depth_postpass`).
fn build(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    velocity: bool,
) -> wgpu::RenderPipeline {
    let (label, vertex, fragment) = match velocity {
        false => ("Depth Postpass Pipeline", "vs_main", "fs_prepass"),
        true => (
            "Depth Postpass Velocity Pipeline",
            "vs_velocity",
            "fs_depth_postpass",
        ),
    };
    let targets = [None, Some(velocity_target(wgpu::ColorWrites::ALL))];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: if velocity { &targets } else { &[] },
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
