//! Render-pipeline construction for the main forward renderer. The bind-group
//! layouts these pipelines reference live in `bind_layouts` (behavior unchanged).

use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::DEPTH_FORMAT;

mod lazy;
mod specs;
pub(crate) mod surface;
pub(crate) mod winding;

pub(crate) use lazy::LazyPipeline;

use specs::{
    forward_spec, line_spec, make_pipeline, transparent_spec, velocity_spec, PipelineSpec,
};

pub(crate) use specs::velocity_target;

/// The forward-target pipelines, all sharing one shader + vertex layout. The ones a
/// frame may not draw (translucents, debug lines) are built on first use (#985); the
/// opaque pass and its prepass are built up front.
pub(crate) struct ForwardPipelines {
    /// Opaque/cutout solids: REPLACE, depth write on.
    pub forward: wgpu::RenderPipeline,
    /// The same, writing the view's velocity target too (#1435): built the first
    /// time a view runs motion blur.
    pub velocity: LazyPipeline,
    /// Alpha-blended translucent solids (#242): ALPHA_BLENDING, depth write off.
    pub transparent: LazyPipeline,
    /// The editor's line gizmos (grid, collider boxes, axis arrows, probes), drawn
    /// over the post-FX output (#1224), so built against the output format.
    pub line: LazyPipeline,
    /// The SSAO depth prepass (#436): the forward vertex stage, no colour target.
    pub prepass: wgpu::RenderPipeline,
    /// Their layout, groups 0–3, which the selection mask (#1175) draws through too.
    pub layout: wgpu::PipelineLayout,
}

/// Builds the forward-lit and prepass pipelines into `format` (the HDR target), and
/// the transparent and line debug pipelines to be built on first use; the lines into
/// `output_format`, the post-FX output they draw over (#1224).
// wgpu pipeline construction legitimately threads several distinct GPU resources
// (device, shader, format, four bind-group layouts); bundling them into a struct
// would only add an indirection without clarifying intent.
#[expect(
    clippy::too_many_arguments,
    reason = "pipeline construction threads distinct GPU resources; a bundle struct adds only indirection"
)]
pub(crate) fn create_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    [format, output_format]: [wgpu::TextureFormat; 2],
    camera_lighting_layout: &wgpu::BindGroupLayout,
    entity_bones_layout: &wgpu::BindGroupLayout,
    material_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
) -> ForwardPipelines {
    let (render_layout, line_layout) = create_pipeline_layouts(
        device,
        camera_lighting_layout,
        entity_bones_layout,
        material_layout,
        shadow_layout,
    );
    let lazy = |layout: &wgpu::PipelineLayout, format, spec: PipelineSpec| {
        let (shader, layout) = (shader.clone(), layout.clone());
        LazyPipeline::new(device, move |device| {
            make_pipeline(device, &shader, format, &layout, &spec)
        })
    };
    ForwardPipelines {
        forward: make_pipeline(device, shader, format, &render_layout, &forward_spec()),
        velocity: lazy(&render_layout, format, velocity_spec()),
        transparent: lazy(&render_layout, format, transparent_spec()),
        line: lazy(&line_layout, output_format, line_spec()),
        prepass: make_prepass(device, shader, &render_layout, "fs_prepass"),
        layout: render_layout,
    }
}

/// An authored surface variant's opaque + transparent pipelines (#396): the forward
/// and transparent specs over `shader`, through the forward pipeline layout; its
/// velocity one (#1435), built on first use — and, when its blocks `cut`, its depth
/// pipelines (#648): the prepass through the same layout, the shadow cascades through
/// the shadow pass's clip layout.
pub(crate) fn surface_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    [layout, shadow_layout]: [&wgpu::PipelineLayout; 2],
    cut: bool,
) -> surface::SurfacePipelines {
    use crate::render::passes::shadows::depth_pipeline;
    use crate::shadergen::assemble::{CUT_PREPASS, CUT_SHADOW};
    let (lazy_shader, lazy_layout) = (shader.clone(), layout.clone());
    surface::SurfacePipelines {
        forward: make_pipeline(device, shader, format, layout, &forward_spec()),
        velocity: LazyPipeline::new(device, move |device| {
            let spec = velocity_spec();
            make_pipeline(device, &lazy_shader, format, &lazy_layout, &spec)
        }),
        transparent: make_pipeline(device, shader, format, layout, &transparent_spec()),
        cut: cut.then(|| surface::CutPipelines {
            prepass: make_prepass(device, shader, layout, CUT_PREPASS),
            shadow: depth_pipeline(device, shader, shadow_layout, Some(CUT_SHADOW), false),
            mirrored_shadow: depth_pipeline(device, shader, shadow_layout, Some(CUT_SHADOW), true),
        }),
    }
}

/// The forward passes' pipeline layout — groups 0–3 — for a surface variant (#396).
pub(crate) fn forward_layout(
    device: &wgpu::Device,
    layouts: [&wgpu::BindGroupLayout; 4],
) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Surface Variant Pipeline Layout"),
        bind_group_layouts: &layouts.map(Some),
        immediate_size: 0,
    })
}

/// The forward and line pipelines share the same four bind-group layouts
/// but carry distinct labels, so build both pipeline layouts here.
fn create_pipeline_layouts(
    device: &wgpu::Device,
    camera_lighting_layout: &wgpu::BindGroupLayout,
    entity_bones_layout: &wgpu::BindGroupLayout,
    material_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
) -> (wgpu::PipelineLayout, wgpu::PipelineLayout) {
    let layouts = [
        camera_lighting_layout,
        entity_bones_layout,
        material_layout,
        shadow_layout,
    ];
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &layouts.map(Some),
        immediate_size: 0,
    });
    let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Line Pipeline Layout"),
        bind_group_layouts: &layouts.map(Some),
        immediate_size: 0,
    });
    (render_pipeline_layout, line_pipeline_layout)
}

/// The SSAO depth prepass (#436): the solids' depth alone, through the forward vertex
/// stage (skinning, instancing) and `fragment` — `fs_prepass` (cutout clip, unlit
/// skip), or a cutting variant's `fs_prepass_cut` (#648). Same layout as the forward
/// pass, so it records with the same bind groups.
fn make_prepass(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: &str,
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
        primitive: wgpu::PrimitiveState::default(),
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
#[path = "tests/scrolled_cut_tests.rs"]
mod scrolled_cut_tests;

#[cfg(test)]
#[path = "tests/lazy_tests.rs"]
mod lazy_tests;
