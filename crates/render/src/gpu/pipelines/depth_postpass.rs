//! HDRP's *Transparent Depth Postpass*, tested `LessEqual` so the forward vertex stage's invariant position lands on its own depth.

use super::{velocity_target, FacedPipeline, Facing, LazyPipeline};
use crate::gpu::mesh::vertex_layout;
use crate::gpu::DEPTH_FORMAT;

pub(crate) struct DepthPostpassPipelines {
    plain: FacedPipeline,
    velocity: FacedPipeline,
    motion: FacedPipeline,
}

impl DepthPostpassPipelines {
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

    pub(crate) fn motion(&self, facing: Facing) -> &wgpu::RenderPipeline {
        self.motion.get(facing)
    }

    pub(crate) fn get(&self, velocity: bool, facing: Facing) -> &wgpu::RenderPipeline {
        match velocity {
            false => self.plain.get(facing),
            true => self.velocity.get(facing),
        }
    }

    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        let depth = self.plain.lazies().chain(self.velocity.lazies());
        depth.chain(self.motion.lazies())
    }
}

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
