//! src/render/ui/pipeline.rs — the UI pass's two pipeline shapes, built in one place
//! (#418, #429, #427).
//!
//! Every UI pipeline — the standard shader's and each custom ui shader's — is one of
//! two shapes over the same vertex format, so only the module and layout vary:
//!
//! - [`UiPass::Screen`] — `vs_main` and a fragment entry (`fs_main`, or #426's
//!   `fs_backdrop`) onto the finished frame (of the given format), no depth;
//! - [`UiPass::World`] — `vs_world`/`fs_world` into the HDR scene target,
//!   depth-tested (`LessEqual`) against the world but never writing depth.
//!
//! Each is built once per blend mode (#425, `blend`): the shader always outputs
//! premultiplied colour and the mode is fixed-function blend state. Neither culls: a clockwise radial fill winds the other way, and a world canvas
//! reads from both sides.

use super::blend::blend_state;
use super::vertex::vertex_layout;
use crate::components::UiBlend;
use crate::render::postfx::HDR_FORMAT;

/// Which UI pass a pipeline draws in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum UiPass {
    /// Screen canvases, onto a frame of this format.
    Screen(wgpu::TextureFormat),
    /// World canvases, into the HDR scene target.
    World,
}

/// The `pass` pipeline drawing `module` through `layout`, compositing with `mode`.
pub(crate) fn build(
    device: &wgpu::Device,
    (label, module, layout): (&str, &wgpu::ShaderModule, &wgpu::PipelineLayout),
    pass: UiPass,
    mode: UiBlend,
) -> wgpu::RenderPipeline {
    build_entry(device, (label, module, layout), pass, (mode, "fs_main"))
}

/// [`build`] through fragment entry `fs` on a screen pass (`fs_world` always in the
/// world pass).
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
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
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
            entry_point: vs,
            buffers: &[vertex_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: fs,
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(blend_state(mode)),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: depth,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}
