//! The forward-pass pipelines' fixed-function knobs and the one function that turns
//! a [`PipelineSpec`] into a pipeline. The layouts and entry points are shared, so
//! each pipeline names only what it changes from [`PipelineSpec::opaque`].

use super::winding;
use crate::render::gpu::mesh::{outline_normal_layout, vertex_layout};
use crate::render::gpu::DEPTH_FORMAT;

/// Fixed-function knobs that distinguish the forward-pass pipelines. The default is
/// the opaque forward pass: no culling, counter-clockwise front faces, REPLACE, depth
/// written and tested `Less`.
#[derive(Clone, Copy)]
pub(super) struct PipelineSpec {
    pub label: &'static str,
    pub topology: wgpu::PrimitiveTopology,
    pub cull_mode: Option<wgpu::Face>,
    pub front_face: wgpu::FrontFace,
    pub blend: wgpu::BlendState,
    pub depth_write_enabled: bool,
    pub depth_compare: wgpu::CompareFunction,
    /// Whether the pipeline also reads the outline normals at vertex slot 1 (#1175).
    pub outline_normals: bool,
}

impl PipelineSpec {
    /// The opaque forward pass under `label`; a spec names only what it changes. The
    /// label has no default, so a spec that forgets one does not compile.
    fn opaque(label: &'static str) -> Self {
        Self {
            label,
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            front_face: wgpu::FrontFace::Ccw,
            blend: wgpu::BlendState::REPLACE,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            outline_normals: false,
        }
    }
}

/// Opaque/cutout forward pass: REPLACE, depth write on. No culling so primitives can
/// render inside-out if needed.
pub(super) fn forward_spec() -> PipelineSpec {
    PipelineSpec::opaque("Forward Lit Pipeline")
}

/// Transparent pass (#242): same shader/layout as the forward pass but alpha-blended
/// and depth-WRITE-off (still depth-TESTED against opaque) so overlapping translucent
/// surfaces all blend rather than z-cull each other. Drawn back-to-front after opaque.
pub(super) fn transparent_spec() -> PipelineSpec {
    PipelineSpec {
        blend: wgpu::BlendState::ALPHA_BLENDING,
        depth_write_enabled: false,
        ..PipelineSpec::opaque("Transparent Pipeline")
    }
}

/// Line debug overlays: don't write depth so they display over the grid.
pub(super) fn line_spec() -> PipelineSpec {
    PipelineSpec {
        topology: wgpu::PrimitiveTopology::LineList,
        blend: wgpu::BlendState::ALPHA_BLENDING,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::LessEqual,
        ..PipelineSpec::opaque("Line Debug Pipeline")
    }
}

/// Selection silhouette (inverted hull): cull front faces so only back faces show.
/// A `mirrored` selection's winding is reversed, so its twin takes clockwise
/// triangles as front (#1142); otherwise its outer shell would cover the object.
/// The hull grows along the outline normals (#1175).
pub(super) fn outline_spec(mirrored: bool) -> PipelineSpec {
    PipelineSpec {
        cull_mode: Some(wgpu::Face::Front),
        outline_normals: true,
        front_face: winding::front_face(mirrored),
        depth_compare: wgpu::CompareFunction::LessEqual,
        ..PipelineSpec::opaque(if mirrored {
            "Mirrored Outline Pipeline"
        } else {
            "Outline Pipeline"
        })
    }
}

/// Create one forward-pass render pipeline from a [`PipelineSpec`]. The five
/// pipelines share the same vertex layout, shader entry points, and depth format;
/// only the fixed-function knobs in the spec differ.
pub(super) fn make_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    spec: &PipelineSpec,
) -> wgpu::RenderPipeline {
    let buffers = [Some(vertex_layout()), Some(outline_normal_layout())];
    let buffers = &buffers[..if spec.outline_normals { 2 } else { 1 }];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(spec.blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: spec.topology,
            front_face: spec.front_face,
            cull_mode: spec.cull_mode,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(spec.depth_write_enabled),
            depth_compare: Some(spec.depth_compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
