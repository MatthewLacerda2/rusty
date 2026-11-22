use super::{FacedPipeline, Facing, LazyPipeline};
use crate::gpu::groups::Groups;
use crate::gpu::mesh::vertex_layout;
use crate::gpu::DEPTH_FORMAT;

pub(super) fn prepass_set(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: &'static str,
) -> FacedPipeline {
    let set = FacedPipeline::lazy(device, shader, layout, move |d, s, l, f| {
        make_prepass(d, s, l, Some(fragment), f)
    });
    set.get(Facing::Ccw);
    set
}

pub(crate) struct PrepassPipelines {
    pub clip: FacedPipeline,
    pub depth_only: FacedPipeline,
}

impl PrepassPipelines {
    pub(super) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        groups: Groups<&wgpu::BindGroupLayout>,
    ) -> Self {
        let plain = depth_only_layout(device, groups);
        Self {
            clip: prepass_set(device, shader, layout, "fs_prepass"),
            depth_only: depth_only_set(device, shader, &plain),
        }
    }

    pub(crate) fn prime(&self) {
        self.depth_only.get(Facing::Ccw);
    }

    pub(crate) fn twins(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.clip.twins().chain(self.depth_only.twins())
    }
}

/// Same vertex stage, primitive and depth state as [`prepass_set`]'s, so the depth is bit-identical.
fn depth_only_set(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
) -> FacedPipeline {
    FacedPipeline::lazy(device, shader, layout, |d, s, l, f| {
        make_prepass(d, s, l, None, f)
    })
}

/// Gaps stand where the material and scene groups sit, keeping `vs_main`'s group indices.
fn depth_only_layout(
    device: &wgpu::Device,
    groups: Groups<&wgpu::BindGroupLayout>,
) -> wgpu::PipelineLayout {
    let groups = Groups {
        scene: None,
        material: None,
        ..groups
    };
    groups.layout(device, "Depth-Only Prepass Layout")
}

fn make_prepass(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: Option<&str>,
    facing: Facing,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Depth Prepass Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: fragment.map(|entry| wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[],
        }),
        primitive: facing.primitive(wgpu::PrimitiveState::default()),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
#[path = "tests/depth_only_tests.rs"]
mod depth_only_tests;
