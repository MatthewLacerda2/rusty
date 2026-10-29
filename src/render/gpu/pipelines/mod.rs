//! Render-pipeline construction for the main forward renderer. The bind-group
//! layouts these pipelines reference live in `bind_layouts` (behavior unchanged).

use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::DEPTH_FORMAT;

mod lazy;
pub(crate) mod priming;
mod specs;
pub(crate) mod surface;
pub(crate) mod transparent;
pub(crate) mod winding;

pub(crate) use lazy::LazyPipeline;
pub(crate) use transparent::{TransparentBlend, TransparentPipelines};

use specs::{
    forward_spec, line_spec, make_pipeline, primed_spec, primed_velocity_spec, transparent_spec,
    velocity_spec, PipelineSpec,
};

pub(crate) use specs::velocity_target;

/// The forward-target pipelines, all sharing one shader + vertex layout. The ones a
/// frame may not draw (translucents, debug lines) are built on first use (#985); the
/// opaque pass and its prepass are built up front.
pub(crate) struct ForwardPipelines {
    /// Opaque/cutout solids: REPLACE, depth write on.
    pub forward: wgpu::RenderPipeline,
    /// The same, writing the view's velocity target too (#1435), and its SSR surface
    /// target (#1526): built the first time a view runs motion blur or SSR.
    pub velocity: VelocityPipelines,
    /// Blended translucent solids (#242), one per blend state (#1523), depth write off.
    pub transparent: TransparentPipelines,
    /// The editor's line gizmos (grid, collider boxes, axis arrows, probes), drawn
    /// over the post-FX output (#1224), so built against the output format.
    pub line: LazyPipeline,
    /// The SSAO depth prepass (#436): the forward vertex stage, no colour target.
    pub prepass: wgpu::RenderPipeline,
    /// The opaque and velocity ones over a primed depth (#1499).
    pub primed: PrimedPipelines,
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
    let lazy = |layout, format, spec| lazy_pipeline(device, shader, format, layout, spec);
    ForwardPipelines {
        forward: make_pipeline(device, shader, format, &render_layout, &forward_spec()),
        velocity: VelocityPipelines::new(|surface| {
            lazy(&render_layout, format, velocity_spec(surface))
        }),
        transparent: TransparentPipelines::new(|blend| {
            lazy(&render_layout, format, transparent_spec(blend))
        }),
        line: lazy(&line_layout, output_format, line_spec()),
        prepass: make_prepass(device, shader, &render_layout, "fs_prepass"),
        primed: PrimedPipelines::new(device, shader, format, &render_layout),
        layout: render_layout,
    }
}

/// An authored surface variant's opaque + transparent pipelines (#396): the forward
/// and transparent specs over `shader`, through the forward pipeline layout; its
/// velocity and primed ones (#1435, #1499), built on first use — and, when its blocks `cut`, its depth
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
    surface::SurfacePipelines {
        forward: make_pipeline(device, shader, format, layout, &forward_spec()),
        velocity: VelocityPipelines::new(|surface| {
            lazy_pipeline(device, shader, format, layout, velocity_spec(surface))
        }),
        primed: PrimedPipelines::new(device, shader, format, layout),
        transparent: TransparentPipelines::new(|blend| {
            lazy_pipeline(device, shader, format, layout, transparent_spec(blend))
        }),
        cut: cut.then(|| surface::CutPipelines {
            prepass: make_prepass(device, shader, layout, CUT_PREPASS),
            shadow: depth_pipeline(device, shader, shadow_layout, Some(CUT_SHADOW), false),
            mirrored_shadow: depth_pipeline(device, shader, shadow_layout, Some(CUT_SHADOW), true),
        }),
    }
}

/// The opaque pipelines over a primed depth (#1499): the SSAO prepass wrote the lit
/// solids' depth, so these test it `LessEqual` and write none. Built the first time
/// a view primes, since a view with AO off never does.
pub(crate) struct PrimedPipelines {
    pub forward: LazyPipeline,
    /// Writing motion vectors too (#1435), and the SSR surface (#1526).
    pub velocity: VelocityPipelines,
}

/// The opaque pipelines a view writing its velocity target draws with (#1435): one
/// into the velocity target alone, one into the SSR surface target too (#1526), each
/// built on first use.
pub(crate) struct VelocityPipelines {
    plain: LazyPipeline,
    surface: LazyPipeline,
}

impl VelocityPipelines {
    /// The pair `make(surface)` builds.
    fn new(make: impl Fn(bool) -> LazyPipeline) -> Self {
        Self {
            plain: make(false),
            surface: make(true),
        }
    }

    /// The one for a view that writes its SSR `surface` target, or not.
    pub(crate) fn get(&self, surface: bool) -> &wgpu::RenderPipeline {
        match surface {
            false => self.plain.get(),
            true => self.surface.get(),
        }
    }

    /// Both, for warming.
    pub(crate) fn lazies(&self) -> [&LazyPipeline; 2] {
        [&self.plain, &self.surface]
    }
}

impl PrimedPipelines {
    /// `shader`'s primed pipelines into `format` through `layout`, not yet built.
    fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        layout: &wgpu::PipelineLayout,
    ) -> Self {
        Self {
            forward: lazy_pipeline(device, shader, format, layout, primed_spec()),
            velocity: VelocityPipelines::new(|surface| {
                lazy_pipeline(
                    device,
                    shader,
                    format,
                    layout,
                    primed_velocity_spec(surface),
                )
            }),
        }
    }
}

/// `spec`'s pipeline over `shader`, built on its first draw (#985).
fn lazy_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    spec: PipelineSpec,
) -> LazyPipeline {
    let (shader, layout) = (shader.clone(), layout.clone());
    LazyPipeline::new(device, move |device| {
        make_pipeline(device, &shader, format, &layout, &spec)
    })
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
/// stage (skinning, instancing) and `fragment` — `fs_prepass` (cutout clip), or the
/// `fs_prepass_cut` of a variant that cuts, moves its UVs or moves its vertices
/// (#648, #1499, #1524). Same layout as the forward pass, so it records with the same
/// bind groups.
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

#[cfg(test)]
#[path = "tests/vertex_waves_tests.rs"]
mod vertex_waves_tests;

#[cfg(test)]
#[path = "tests/priming/backend_tests.rs"]
mod priming_backend_tests;
