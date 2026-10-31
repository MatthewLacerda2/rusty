//! The forward-pass pipelines' fixed-function knobs and the one function that turns
//! a [`PipelineSpec`] into a pipeline. The layouts and entry points are shared, so
//! each pipeline names only what it changes from [`PipelineSpec::opaque`].

use super::transparent::TransparentBlend;
use super::winding::Facing;
use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::DEPTH_FORMAT;
use crate::render::postfx::effects::ssr::surface::target_state as surface_target_state;

/// Fixed-function knobs that distinguish the forward-pass pipelines. The default is
/// the opaque forward pass: counter-clockwise triangles kept (each pass builds one
/// per [`Facing`], #1615), REPLACE, depth written and tested `Less`.
#[derive(Clone, Copy)]
pub(super) struct PipelineSpec {
    pub label: &'static str,
    pub topology: wgpu::PrimitiveTopology,
    /// Which triangles it keeps: a material's render face, by the mesh's winding.
    pub facing: Facing,
    pub blend: wgpu::BlendState,
    pub depth_write_enabled: bool,
    pub depth_compare: wgpu::CompareFunction,
    /// The vertex entry point: `vs_main`, or the motion-vector `vs_velocity` (#1435).
    pub vertex: &'static str,
    /// The fragment entry point: `fs_main`, the gizmos' unfogged `fs_gizmo`, the
    /// motion-vector `fs_velocity`, or `fs_velocity_ssr`, which writes the SSR surface
    /// targets too (#1526).
    pub fragment: &'static str,
    /// The velocity target's write mask when the pipeline draws into a view's
    /// velocity target too (#1435), as its second colour target; `None` draws colour
    /// alone.
    pub velocity: Option<wgpu::ColorWrites>,
    /// Whether it draws into a view's two SSR surface targets too (#1526), as its
    /// third and fourth colour targets, after the velocity one.
    pub surface: bool,
}

impl PipelineSpec {
    /// The opaque forward pass under `label`; a spec names only what it changes. The
    /// label has no default, so a spec that forgets one does not compile.
    fn opaque(label: &'static str) -> Self {
        Self {
            label,
            topology: wgpu::PrimitiveTopology::TriangleList,
            facing: Facing::Ccw,
            blend: wgpu::BlendState::REPLACE,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            vertex: "vs_main",
            fragment: "fs_main",
            velocity: None,
            surface: false,
        }
    }

    /// This spec keeping `facing`'s triangles.
    pub(super) fn facing(self, facing: Facing) -> Self {
        Self { facing, ..self }
    }
}

/// Opaque/cutout forward pass: REPLACE, depth write on.
pub(super) fn forward_spec() -> PipelineSpec {
    PipelineSpec::opaque("Forward Lit Pipeline")
}

/// The opaque pass of a view writing motion vectors (#1435): colour and velocity
/// from the shader's `vs_velocity` / `fs_velocity`, which every surface module has;
/// with `surface`, a view running SSR, its surface target too through
/// `fs_velocity_ssr` (#1526).
pub(super) fn velocity_spec(surface: bool) -> PipelineSpec {
    let (label, fragment) = match surface {
        false => ("Forward Velocity Pipeline", "fs_velocity"),
        true => ("Forward Velocity SSR Pipeline", "fs_velocity_ssr"),
    };
    PipelineSpec {
        vertex: "vs_velocity",
        fragment,
        velocity: Some(wgpu::ColorWrites::ALL),
        surface,
        ..PipelineSpec::opaque(label)
    }
}

/// The opaque pass over a primed depth (#1499), Unity URP's *Depth Priming*: the
/// SSAO prepass already wrote these solids' depth, so the pass tests it and writes
/// none, and the fragment shader runs once per visible pixel. With no depth write, a
/// GPU tests depth before a shader that discards (a cutout) too. URP tests `Equal`;
/// with the vertex position `@invariant`, `LessEqual` passes the same fragments, and
/// still draws one a driver rounds a step nearer than its prepass depth. Two lit
/// surfaces at bit-identical depth both pass, so the one drawn last wins, as under
/// URP's `Equal`, where the unprimed pass's `Less` kept the first.
pub(super) fn primed_spec() -> PipelineSpec {
    primed(PipelineSpec::opaque("Forward Primed Pipeline"))
}

/// [`velocity_spec`] over a primed depth (#1499), as [`primed_spec`] is.
pub(super) fn primed_velocity_spec(surface: bool) -> PipelineSpec {
    let label = match surface {
        false => "Forward Primed Velocity Pipeline",
        true => "Forward Primed Velocity SSR Pipeline",
    };
    PipelineSpec {
        label,
        ..primed(velocity_spec(surface))
    }
}

/// `spec` testing a primed depth (#1499): `LessEqual`, no depth write.
fn primed(spec: PipelineSpec) -> PipelineSpec {
    PipelineSpec {
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::LessEqual,
        ..spec
    }
}

/// Transparent pass (#242): same shader/layout as the forward pass but blended by
/// `blend` (#1523) and depth-WRITE-off (still depth-TESTED against opaque) so
/// overlapping translucent surfaces all blend rather than z-cull each other. Drawn
/// back-to-front after opaque.
pub(super) fn transparent_spec(blend: TransparentBlend) -> PipelineSpec {
    PipelineSpec {
        blend: blend.state(),
        depth_write_enabled: false,
        ..PipelineSpec::opaque(blend.label())
    }
}

/// The editor's line gizmos: don't write depth so they display over the grid. Drawn
/// on the finished image after post-FX (#1224), so `fs_gizmo` writes the authored
/// colour unfogged, as Unity draws its gizmos. Lines have no face, so nothing culls.
pub(super) fn line_spec() -> PipelineSpec {
    PipelineSpec {
        facing: Facing::Both,
        fragment: "fs_gizmo",
        topology: wgpu::PrimitiveTopology::LineList,
        blend: wgpu::BlendState::ALPHA_BLENDING,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::LessEqual,
        ..PipelineSpec::opaque("Line Debug Pipeline")
    }
}

/// The velocity target of a pipeline that writes motion vectors (#1435), under
/// `write_mask`. It is never blended: the nearest surface's motion wins.
pub(crate) fn velocity_target(write_mask: wgpu::ColorWrites) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: crate::render::motion::VELOCITY_FORMAT,
        blend: None,
        write_mask,
    }
}

/// Create one forward-pass render pipeline from a [`PipelineSpec`]. The pipelines
/// share the same vertex layout and depth format; only the spec's fixed-function
/// knobs, entry points and velocity and surface targets differ.
pub(super) fn make_pipeline(
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
            entry_point: Some(spec.vertex),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(spec.fragment),
            compilation_options: Default::default(),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(spec.blend),
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                spec.velocity.map(velocity_target),
                Some(surface_target_state(wgpu::ColorWrites::ALL)),
                Some(surface_target_state(wgpu::ColorWrites::ALL)),
            ][..1 + usize::from(spec.velocity.is_some()) + 2 * usize::from(spec.surface)],
        }),
        primitive: spec.facing.primitive(wgpu::PrimitiveState {
            topology: spec.topology,
            ..Default::default()
        }),
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
