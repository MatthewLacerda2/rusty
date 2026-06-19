//! Render-pipeline construction for the main forward renderer. The bind-group
//! layouts these pipelines reference live in `bind_layouts` (behavior unchanged).

use super::mesh::Vertex;

/// Fixed-function knobs that distinguish the three forward-pass pipelines.
struct PipelineSpec {
    label: &'static str,
    topology: wgpu::PrimitiveTopology,
    cull_mode: Option<wgpu::Face>,
    blend: wgpu::BlendState,
    depth_write_enabled: bool,
    depth_compare: wgpu::CompareFunction,
}

/// Builds the forward-lit, line debug, and outline pipelines.
// wgpu pipeline construction legitimately threads several distinct GPU resources
// (device, shader, format, four bind-group layouts); bundling them into a struct
// would only add an indirection without clarifying intent.
#[allow(clippy::too_many_arguments)]
pub(super) fn create_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    camera_lighting_layout: &wgpu::BindGroupLayout,
    entity_bones_layout: &wgpu::BindGroupLayout,
    material_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
) {
    let (render_pipeline_layout, line_pipeline_layout) = create_pipeline_layouts(
        device,
        camera_lighting_layout,
        entity_bones_layout,
        material_layout,
        shadow_layout,
    );

    let render_pipeline = make_pipeline(
        device,
        shader,
        format,
        &render_pipeline_layout,
        &PipelineSpec {
            label: "Forward Lit Pipeline",
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None, // No culling to easily render primitives inside-out if needed
            blend: wgpu::BlendState::REPLACE,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
        },
    );

    let line_pipeline = make_pipeline(
        device,
        shader,
        format,
        &line_pipeline_layout,
        &PipelineSpec {
            label: "Line Debug Pipeline",
            topology: wgpu::PrimitiveTopology::LineList,
            cull_mode: None,
            blend: wgpu::BlendState::ALPHA_BLENDING,
            // Don't write depth for line overlays so they display over the grid.
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
        },
    );

    // Outline pipeline for selection silhouette (inverted hull technique).
    let outline_pipeline = make_pipeline(
        device,
        shader,
        format,
        &render_pipeline_layout,
        &PipelineSpec {
            label: "Outline Pipeline",
            topology: wgpu::PrimitiveTopology::TriangleList,
            // Cull front faces so only back faces (the outline) show.
            cull_mode: Some(wgpu::Face::Front),
            blend: wgpu::BlendState::REPLACE,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
        },
    );

    (render_pipeline, line_pipeline, outline_pipeline)
}

/// The forward/outline and line pipelines share the same four bind-group layouts
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
        bind_group_layouts: &layouts,
        push_constant_ranges: &[],
    });
    let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Line Pipeline Layout"),
        bind_group_layouts: &layouts,
        push_constant_ranges: &[],
    });
    (render_pipeline_layout, line_pipeline_layout)
}

/// Create one forward-pass render pipeline from a [`PipelineSpec`]. All three
/// passes share the same vertex layout, shader entry points, and depth format;
/// only the fixed-function knobs in the spec differ.
fn make_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    spec: &PipelineSpec,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: &[Vertex::desc()],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(spec.blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: spec.topology,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: spec.cull_mode,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: spec.depth_write_enabled,
            depth_compare: spec.depth_compare,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}
