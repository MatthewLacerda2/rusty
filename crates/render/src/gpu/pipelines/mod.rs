//! Render-pipeline construction for the main forward renderer. The bind-group
//! layouts these pipelines reference live in `bind_layouts` (behavior unchanged).

mod depth_postpass;
mod lazy;
mod prepass;
pub(crate) mod priming;
mod specs;
pub(crate) mod surface;
pub(crate) mod transparent;
pub(crate) mod winding;

pub(crate) use depth_postpass::DepthPostpassPipelines;
use lazy::lazy_pipeline;
pub(crate) use lazy::{lazy_compute, LazyCompute, LazyPipeline};
pub(crate) use transparent::{TransparentBlend, TransparentPipelines};
pub(crate) use winding::{FacedPipeline, Facing};

use crate::gpu::groups::Groups;
use prepass::prepass_set;
use specs::{
    forward_spec, line_spec, primed_spec, primed_velocity_spec, transparent_spec, velocity_spec,
    vertex_line_spec, PipelineSpec,
};

pub(crate) use specs::velocity_target;

/// The forward-target pipelines, all sharing one shader + vertex layout, each pass
/// one per [`Facing`] (#1615). The ones a frame may not draw (translucents, debug
/// lines, a mirrored or double-sided mesh's) are built on first use (#985); the
/// opaque pass and its prepass are built up front for the usual facing.
pub(crate) struct ForwardPipelines {
    /// Opaque/cutout solids: REPLACE, depth write on.
    pub forward: FacedPipeline,
    /// The same, writing the view's velocity target too (#1435), and its SSR surface
    /// target (#1526): built the first time a view runs motion blur or SSR.
    pub velocity: VelocityPipelines,
    /// Blended translucent solids (#242), one per blend state (#1523), depth write off.
    pub transparent: TransparentPipelines,
    /// The translucents' depth postpass (#1523).
    pub depth_postpass: DepthPostpassPipelines,
    /// The editor's line gizmos (grid, axis arrows, probes, the occlusion view), drawn
    /// over the post-FX output (#1224), so built against the output format.
    pub line: LazyPipeline,
    /// The collider and debug-line gizmos (#1882): lines coloured per vertex.
    pub vertex_line: LazyPipeline,
    /// The SSAO depth prepass (#436): the forward vertex stage, no colour target.
    pub prepass: FacedPipeline,
    /// The opaque and velocity ones over a primed depth (#1499).
    pub primed: PrimedPipelines,
    /// Their layout ([`forward_groups`]), which the selection mask (#1175) draws through too.
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
    let groups = forward_groups(
        camera_lighting_layout,
        entity_bones_layout,
        material_layout,
        shadow_layout,
    );
    let (render_layout, line_layout) = create_pipeline_layouts(device, groups);
    let set = SpecSet::new(device, shader, format, &render_layout);
    ForwardPipelines {
        forward: FacedPipeline::eager(|f| set.lazy(forward_spec().facing(f))),
        velocity: VelocityPipelines::new(|surface| set.faced(velocity_spec(surface))),
        transparent: TransparentPipelines::new(|blend, reactive| {
            set.faced(transparent_spec(blend, reactive))
        }),
        depth_postpass: DepthPostpassPipelines::new(device, shader, &render_layout),
        line: lazy_pipeline(device, shader, output_format, &line_layout, line_spec()),
        vertex_line: lazy_pipeline(
            device,
            shader,
            output_format,
            &line_layout,
            vertex_line_spec(),
        ),
        prepass: prepass_set(device, shader, &render_layout, "fs_prepass"),
        primed: PrimedPipelines::new(&set),
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
    use crate::passes::shadows::{depth_pipeline, lazy_depth_pipeline};
    use rusty_shadergen::assemble::{CUT_PREPASS, CUT_SHADOW};
    let shadow = |facing| depth_pipeline(device, shader, shadow_layout, Some(CUT_SHADOW), facing);
    let lazy = |f| lazy_depth_pipeline(device, shader, shadow_layout, Some(CUT_SHADOW), f);
    let set = SpecSet::new(device, shader, format, layout);
    surface::SurfacePipelines {
        forward: FacedPipeline::eager(|f| set.lazy(forward_spec().facing(f))),
        velocity: VelocityPipelines::new(|surface| set.faced(velocity_spec(surface))),
        primed: PrimedPipelines::new(&set),
        transparent: TransparentPipelines::new(|blend, reactive| {
            set.faced(transparent_spec(blend, reactive))
        }),
        depth_postpass: DepthPostpassPipelines::new(device, shader, layout),
        cut: cut.then(|| surface::CutPipelines {
            prepass: prepass_set(device, shader, layout, CUT_PREPASS),
            shadow: shadow(Facing::Ccw),
            mirrored_shadow: shadow(Facing::Cw),
            two_sided_shadow: lazy(Facing::Both),
        }),
    }
}

/// What every forward-target pipeline of one shader shares: the device, the shader,
/// the colour format and the layout, so a pass names only its spec.
struct SpecSet<'a> {
    device: &'a wgpu::Device,
    shader: &'a wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    layout: &'a wgpu::PipelineLayout,
}

impl<'a> SpecSet<'a> {
    fn new(
        device: &'a wgpu::Device,
        shader: &'a wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        layout: &'a wgpu::PipelineLayout,
    ) -> Self {
        Self {
            device,
            shader,
            format,
            layout,
        }
    }

    /// `spec`'s pipeline, built on its first draw.
    fn lazy(&self, spec: PipelineSpec) -> LazyPipeline {
        lazy_pipeline(self.device, self.shader, self.format, self.layout, spec)
    }

    /// `spec`'s pipeline per [`Facing`], each built on its first draw.
    fn faced(&self, spec: PipelineSpec) -> FacedPipeline {
        FacedPipeline::new(|f| self.lazy(spec.facing(f)))
    }
}

/// The opaque pipelines over a primed depth (#1499): the SSAO prepass wrote the lit
/// solids' depth, so these test it `LessEqual` and write none. Built the first time
/// a view primes, since a view with AO off never does.
pub(crate) struct PrimedPipelines {
    pub forward: FacedPipeline,
    /// Writing motion vectors too (#1435), and the SSR surface (#1526).
    pub velocity: VelocityPipelines,
}

/// The opaque pipelines a view writing its velocity target draws with (#1435): one
/// into the velocity target alone, one into the SSR surface target too (#1526), each
/// one per facing, built on first use.
pub(crate) struct VelocityPipelines {
    plain: FacedPipeline,
    surface: FacedPipeline,
}

impl VelocityPipelines {
    /// The pair `make(surface)` builds.
    fn new(make: impl Fn(bool) -> FacedPipeline) -> Self {
        Self {
            plain: make(false),
            surface: make(true),
        }
    }

    /// The one for a view that writes its SSR `surface` target, or not, keeping
    /// `facing`'s triangles.
    pub(crate) fn get(&self, surface: bool, facing: Facing) -> &wgpu::RenderPipeline {
        match surface {
            false => self.plain.get(facing),
            true => self.surface.get(facing),
        }
    }

    /// Every one, for warming.
    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.plain.lazies().chain(self.surface.lazies())
    }
}

impl PrimedPipelines {
    /// `set`'s primed pipelines, not yet built.
    fn new(set: &SpecSet) -> Self {
        Self {
            forward: set.faced(primed_spec()),
            velocity: VelocityPipelines::new(|surface| set.faced(primed_velocity_spec(surface))),
        }
    }

    /// Every one, for warming.
    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.forward.lazies().chain(self.velocity.lazies())
    }
}

/// The forward passes' groups by role: every forward-target layout (the solids,
/// their prepass, translucents, lines, the selection mask and a surface variant's)
/// places them through this, so all of them share one order ([`Groups`]).
pub(crate) fn forward_groups<T>(global: T, draw: T, material: T, scene: T) -> Groups<T> {
    Groups {
        global,
        scene: Some(scene),
        material: Some(material),
        draw,
    }
}

/// The forward passes' pipeline layout for a surface variant (#396), over
/// [`forward_groups`].
pub(crate) fn forward_layout(
    device: &wgpu::Device,
    groups: Groups<&wgpu::BindGroupLayout>,
) -> wgpu::PipelineLayout {
    groups.layout(device, "Surface Variant Pipeline Layout")
}

/// The forward and line pipelines share the same four bind-group layouts
/// but carry distinct labels, so build both pipeline layouts here.
fn create_pipeline_layouts(
    device: &wgpu::Device,
    groups: Groups<&wgpu::BindGroupLayout>,
) -> (wgpu::PipelineLayout, wgpu::PipelineLayout) {
    (
        groups.layout(device, "Render Pipeline Layout"),
        groups.layout(device, "Line Pipeline Layout"),
    )
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

#[cfg(test)]
#[path = "tests/render_face/face_tests.rs"]
mod render_face_tests;

#[cfg(test)]
#[path = "tests/render_face/normal_tests.rs"]
mod render_face_normal_tests;

#[cfg(test)]
#[path = "tests/render_face/surface_tests.rs"]
mod render_face_surface_tests;
