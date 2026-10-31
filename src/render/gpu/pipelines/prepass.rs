//! The SSAO depth prepass's pipelines (#436), one per [`Facing`] (#1615).

use super::{FacedPipeline, Facing, LazyPipeline};
use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::DEPTH_FORMAT;

/// The SSAO depth prepass (#436): the solids' depth alone, through the forward vertex
/// stage (skinning, instancing) and `fragment` — `fs_prepass` (cutout clip), or the
/// `fs_prepass_cut` of a variant that cuts, moves its UVs or moves its vertices
/// (#648, #1499, #1524) — one per [`Facing`], so it keeps the triangles the opaque
/// pass keeps (#1615); the usual one built now. Same layout as the forward pass, so
/// it records with the same bind groups.
pub(super) fn prepass_set(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: &'static str,
) -> FacedPipeline {
    FacedPipeline::eager(|facing| {
        let (shader, layout) = (shader.clone(), layout.clone());
        LazyPipeline::new(device, move |device| {
            make_prepass(device, &shader, &layout, fragment, facing)
        })
    })
}

/// One [`prepass_set`] pipeline.
fn make_prepass(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: &str,
    facing: Facing,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SSAO Depth Prepass Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
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
