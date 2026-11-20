//! The SSAO depth prepass's pipelines (#436), one per [`Facing`] (#1615): the
//! clipping ones, whose fragment stage can discard, and the depth-only ones with
//! no fragment stage at all, for the batches nothing clips (#2347).

use super::{FacedPipeline, Facing, LazyPipeline};
use crate::gpu::mesh::vertex_layout;
use crate::gpu::DEPTH_FORMAT;

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
    let set = FacedPipeline::lazy(device, shader, layout, move |d, s, l, f| {
        make_prepass(d, s, l, Some(fragment), f)
    });
    set.get(Facing::Ccw);
    set
}

/// The standard shader's prepass pipelines: the clipping `fs_prepass` ones, and the
/// depth-only ones a batch nothing clips draws with (`BatchKey::depth_only`, #2347).
pub(crate) struct PrepassPipelines {
    pub clip: FacedPipeline,
    pub depth_only: FacedPipeline,
}

impl PrepassPipelines {
    /// Both sets of the forward `shader`: the clipping ones over the forward
    /// `layout`, their usual facing built now, and the depth-only ones over one of
    /// the forward pass's `globals` and `draws` groups alone ([`depth_only_layout`]),
    /// none built until [`Self::prime`] or a draw: the shader preview's throwaway
    /// set never draws them.
    pub(super) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        groups: [&wgpu::BindGroupLayout; 2],
    ) -> Self {
        let plain = depth_only_layout(device, groups);
        Self {
            clip: prepass_set(device, shader, layout, "fs_prepass"),
            depth_only: depth_only_set(device, shader, &plain),
        }
    }

    /// Build the depth-only set's usual facing now, as the clipping one's is: the
    /// renderer's own set, which every prepass draws.
    pub(crate) fn prime(&self) {
        self.depth_only.get(Facing::Ccw);
    }

    /// The facings still to build, for warming.
    pub(crate) fn twins(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.clip.twins().chain(self.depth_only.twins())
    }
}

/// The depth-only prepass (#2347): the same vertex stage, primitive and depth state
/// as [`prepass_set`]'s, so the depth it writes is bit-identical, but no fragment
/// stage, so a batch nothing clips pays no fragment work and keeps the GPU's
/// depth-only fast path. `layout` is [`depth_only_layout`]'s, which ends before
/// the material group; nothing is built yet.
fn depth_only_set(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
) -> FacedPipeline {
    FacedPipeline::lazy(device, shader, layout, |d, s, l, f| {
        make_prepass(d, s, l, None, f)
    })
}

/// The depth-only prepass's layout: the forward pass's groups 0 (globals) and 1
/// (draw buffers), which `vs_main` reads, and nothing after, so a depth-only batch
/// binds no material group (2) and the pass's group 3 stays bound for the next
/// clipping batch.
fn depth_only_layout(
    device: &wgpu::Device,
    [globals, draws]: [&wgpu::BindGroupLayout; 2],
) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Depth-Only Prepass Layout"),
        bind_group_layouts: &[Some(globals), Some(draws)],
        immediate_size: 0,
    })
}

/// One prepass pipeline: through `fragment`, or with no fragment stage.
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
