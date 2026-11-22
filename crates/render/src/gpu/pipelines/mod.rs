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
pub(crate) use prepass::PrepassPipelines;
use specs::{
    forward_spec, line_spec, primed_spec, primed_velocity_spec, transparent_spec, velocity_spec,
    vertex_line_spec, PipelineSpec,
};

pub(crate) use specs::velocity_target;

pub(crate) struct ForwardPipelines {
    pub forward: FacedPipeline,
    pub velocity: VelocityPipelines,
    pub transparent: TransparentPipelines,
    pub depth_postpass: DepthPostpassPipelines,
    /// Drawn over the post-FX output, so built against `output_format`.
    pub line: LazyPipeline,
    pub vertex_line: LazyPipeline,
    pub prepass: PrepassPipelines,
    pub primed: PrimedPipelines,
    pub layout: wgpu::PipelineLayout,
}

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
    let groups = Groups {
        global: camera_lighting_layout,
        scene: Some(shadow_layout),
        material: Some(material_layout),
        draw: entity_bones_layout,
    };
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
        prepass: PrepassPipelines::new(device, shader, &render_layout, groups),
        primed: PrimedPipelines::new(&set),
        layout: render_layout,
    }
}

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

    fn lazy(&self, spec: PipelineSpec) -> LazyPipeline {
        lazy_pipeline(self.device, self.shader, self.format, self.layout, spec)
    }

    fn faced(&self, spec: PipelineSpec) -> FacedPipeline {
        FacedPipeline::new(|f| self.lazy(spec.facing(f)))
    }
}

pub(crate) struct PrimedPipelines {
    pub forward: FacedPipeline,
    pub velocity: VelocityPipelines,
}

pub(crate) struct VelocityPipelines {
    plain: FacedPipeline,
    surface: FacedPipeline,
}

impl VelocityPipelines {
    fn new(make: impl Fn(bool) -> FacedPipeline) -> Self {
        Self {
            plain: make(false),
            surface: make(true),
        }
    }

    pub(crate) fn get(&self, surface: bool, facing: Facing) -> &wgpu::RenderPipeline {
        match surface {
            false => self.plain.get(facing),
            true => self.surface.get(facing),
        }
    }

    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.plain.lazies().chain(self.surface.lazies())
    }
}

impl PrimedPipelines {
    fn new(set: &SpecSet) -> Self {
        Self {
            forward: set.faced(primed_spec()),
            velocity: VelocityPipelines::new(|surface| set.faced(primed_velocity_spec(surface))),
        }
    }

    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.forward.lazies().chain(self.velocity.lazies())
    }
}

pub(crate) fn forward_layout(
    device: &wgpu::Device,
    groups: Groups<&wgpu::BindGroupLayout>,
) -> wgpu::PipelineLayout {
    groups.layout(device, "Surface Variant Pipeline Layout")
}

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
