//! The transparent depth postpass's pipelines (#1523), HDRP's *Transparent Depth
//! Postpass*: a translucent surface that asks for it draws again after the
//! translucents, writing only its depth (tested `LessEqual`, so the forward vertex
//! stage's invariant position lands on its own depth) and, in a view with a velocity
//! target, its texel there. A translucent that writes its motion alone (#1845,
//! HDRP's *Transparent Writes Motion Vectors*) draws its texel without its depth.
//! Each is built on its first draw.

use super::{velocity_target, FacedPipeline, Facing, LazyPipeline};
use crate::gpu::mesh::vertex_layout;
use crate::gpu::DEPTH_FORMAT;

/// One shader's depth postpass pipelines: depth alone, depth with the velocity
/// target, and the velocity target alone (#1845), each one per [`Facing`] (#1615).
pub(crate) struct DepthPostpassPipelines {
    plain: FacedPipeline,
    velocity: FacedPipeline,
    motion: FacedPipeline,
}

impl DepthPostpassPipelines {
    /// `shader`'s pair through the forward `layout`, not yet built.
    pub(super) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> Self {
        let make = |velocity: bool, depth: bool| {
            FacedPipeline::lazy(device, shader, layout, move |d, s, l, f| {
                build(d, s, l, (velocity, depth), f)
            })
        };
        Self {
            plain: make(false, true),
            velocity: make(true, true),
            motion: make(true, false),
        }
    }

    /// The one writing the velocity target alone, depth tested but not written
    /// (#1845), keeping `facing`'s triangles.
    pub(crate) fn motion(&self, facing: Facing) -> &wgpu::RenderPipeline {
        self.motion.get(facing)
    }

    /// The one for a view that writes a velocity target, or not, keeping
    /// `facing`'s triangles.
    pub(crate) fn get(&self, velocity: bool, facing: Facing) -> &wgpu::RenderPipeline {
        match velocity {
            false => self.plain.get(facing),
            true => self.velocity.get(facing),
        }
    }

    /// Every one, for warming.
    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        let depth = self.plain.lazies().chain(self.velocity.lazies());
        depth.chain(self.motion.lazies())
    }
}

/// The depth-only pipeline (`vs_main` / `fs_prepass`), or with `velocity` the one
/// writing the velocity target at location 1 (`vs_velocity` / `fs_depth_postpass`),
/// writing depth too when `depth` (the motion-only one, #1845, writes none).
fn build(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    (velocity, depth): (bool, bool),
    facing: Facing,
) -> wgpu::RenderPipeline {
    let (label, vertex, fragment) = match (velocity, depth) {
        (false, _) => ("Depth Postpass Pipeline", "vs_main", "fs_prepass"),
        (true, true) => (
            "Depth Postpass Velocity Pipeline",
            "vs_velocity",
            "fs_depth_postpass",
        ),
        (true, false) => (
            "Transparent Motion Vectors Pipeline",
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
        primitive: facing.primitive(wgpu::PrimitiveState::default()),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(depth),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
