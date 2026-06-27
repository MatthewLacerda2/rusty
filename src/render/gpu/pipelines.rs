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

/// The four forward-target pipelines, all sharing one shader + vertex layout.
pub(super) struct ForwardPipelines {
    /// Opaque/cutout solids: REPLACE, depth write on.
    pub forward: wgpu::RenderPipeline,
    /// Alpha-blended translucent solids (#242): ALPHA_BLENDING, depth write off.
    pub transparent: wgpu::RenderPipeline,
    /// Line debug overlays.
    pub line: wgpu::RenderPipeline,
    /// Selection-silhouette inverted hull.
    pub outline: wgpu::RenderPipeline,
}

/// Builds the forward-lit, transparent, line debug, and outline pipelines.
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
) -> ForwardPipelines {
    let (render_layout, line_layout) = create_pipeline_layouts(
        device,
        camera_lighting_layout,
        entity_bones_layout,
        material_layout,
        shadow_layout,
    );
    let p = |layout: &wgpu::PipelineLayout, spec: &PipelineSpec| {
        make_pipeline(device, shader, format, layout, spec)
    };
    ForwardPipelines {
        forward: p(&render_layout, &forward_spec()),
        transparent: p(&render_layout, &transparent_spec()),
        line: p(&line_layout, &line_spec()),
        outline: p(&render_layout, &outline_spec()),
    }
}

/// Opaque/cutout forward pass: REPLACE, depth write on. No culling so primitives can
/// render inside-out if needed.
fn forward_spec() -> PipelineSpec {
    PipelineSpec {
        label: "Forward Lit Pipeline",
        topology: wgpu::PrimitiveTopology::TriangleList,
        cull_mode: None,
        blend: wgpu::BlendState::REPLACE,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::Less,
    }
}

/// Transparent pass (#242): same shader/layout as the forward pass but alpha-blended
/// and depth-WRITE-off (still depth-TESTED against opaque) so overlapping translucent
/// surfaces all blend rather than z-cull each other. Drawn back-to-front after opaque.
fn transparent_spec() -> PipelineSpec {
    PipelineSpec {
        label: "Transparent Pipeline",
        topology: wgpu::PrimitiveTopology::TriangleList,
        cull_mode: None,
        blend: wgpu::BlendState::ALPHA_BLENDING,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::Less,
    }
}

/// Line debug overlays: don't write depth so they display over the grid.
fn line_spec() -> PipelineSpec {
    PipelineSpec {
        label: "Line Debug Pipeline",
        topology: wgpu::PrimitiveTopology::LineList,
        cull_mode: None,
        blend: wgpu::BlendState::ALPHA_BLENDING,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::LessEqual,
    }
}

/// Selection silhouette (inverted hull): cull front faces so only back faces show.
fn outline_spec() -> PipelineSpec {
    PipelineSpec {
        label: "Outline Pipeline",
        topology: wgpu::PrimitiveTopology::TriangleList,
        cull_mode: Some(wgpu::Face::Front),
        blend: wgpu::BlendState::REPLACE,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
    }
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
