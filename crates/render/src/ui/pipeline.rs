use super::blend::blend_state;
use super::vertex::vertex_layout;
use crate::gpu::DEPTH_FORMAT;
use crate::postfx::HDR_FORMAT;
use rusty_components::UiBlend;

pub(crate) const SOURCE_GROUP: u32 = 0;
pub(crate) const BATCH_GROUP: u32 = 1;
pub(crate) const CANVAS_GROUP: u32 = 2;
pub(crate) const SHADE_GROUP: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum UiPass {
    Screen(wgpu::TextureFormat),
    World,
}

pub(crate) fn build(
    device: &wgpu::Device,
    (label, module, layout): (&str, &wgpu::ShaderModule, &wgpu::PipelineLayout),
    pass: UiPass,
    mode: UiBlend,
) -> wgpu::RenderPipeline {
    build_entry(device, (label, module, layout), pass, (mode, "fs_main"))
}

pub(crate) fn build_entry(
    device: &wgpu::Device,
    (label, module, layout): (&str, &wgpu::ShaderModule, &wgpu::PipelineLayout),
    pass: UiPass,
    (mode, fs): (UiBlend, &str),
) -> wgpu::RenderPipeline {
    let (vs, fs, format, depth) = match pass {
        UiPass::Screen(format) => ("vs_main", fs, format, None),
        UiPass::World => (
            "vs_world",
            "fs_world",
            HDR_FORMAT,
            Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        ),
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vs),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(blend_state(mode)),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: depth,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
