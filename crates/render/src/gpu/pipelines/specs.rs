use super::transparent::TransparentBlend;
use super::winding::Facing;
use crate::gpu::mesh::vertex_layout;
use crate::gpu::DEPTH_FORMAT;
use crate::postfx::effects::ssr::surface::target_state as surface_target_state;
use crate::postfx::effects::taa::reactive;

#[derive(Clone, Copy)]
pub(super) struct PipelineSpec {
    pub label: &'static str,
    pub topology: wgpu::PrimitiveTopology,
    pub facing: Facing,
    pub blend: wgpu::BlendState,
    pub depth_write_enabled: bool,
    pub depth_compare: wgpu::CompareFunction,
    pub vertex: &'static str,
    pub fragment: &'static str,
    pub velocity: Option<wgpu::ColorWrites>,
    pub surface: bool,
    pub reactive: bool,
}

impl PipelineSpec {
    fn opaque(label: &'static str) -> Self {
        Self {
            label,
            topology: wgpu::PrimitiveTopology::TriangleList,
            facing: Facing::Ccw,
            blend: wgpu::BlendState::REPLACE,
            depth_write_enabled: true,
            depth_compare: crate::gpu::depth::NEARER,
            vertex: "vs_main",
            fragment: "fs_main",
            velocity: None,
            surface: false,
            reactive: false,
        }
    }

    pub(super) fn facing(self, facing: Facing) -> Self {
        Self { facing, ..self }
    }
}

pub(super) fn forward_spec() -> PipelineSpec {
    PipelineSpec::opaque("Forward Lit Pipeline")
}

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

/// URP tests `Equal`; with the vertex position `@invariant`, `LessEqual` passes the same fragments
/// and still draws one a driver rounds a step nearer. Two lit surfaces at identical depth both
/// pass, so the last drawn wins.
pub(super) fn primed_spec() -> PipelineSpec {
    primed(PipelineSpec::opaque("Forward Primed Pipeline"))
}

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

fn primed(spec: PipelineSpec) -> PipelineSpec {
    PipelineSpec {
        depth_write_enabled: false,
        depth_compare: crate::gpu::depth::NEARER_OR_EQUAL,
        ..spec
    }
}

/// Depth tested but not written, so overlapping translucents all blend instead of culling.
pub(super) fn transparent_spec(blend: TransparentBlend, reactive: bool) -> PipelineSpec {
    PipelineSpec {
        blend: blend.state(),
        depth_write_enabled: false,
        fragment: if reactive {
            "fs_transparent"
        } else {
            "fs_main"
        },
        reactive,
        ..PipelineSpec::opaque(blend.label())
    }
}

/// SSR's *Mixed* translucent surface (`fs_ssr_transparent`): the nearest wins, into its own depth.
pub(super) fn ssr_surface_spec() -> PipelineSpec {
    PipelineSpec {
        fragment: "fs_ssr_transparent",
        ..PipelineSpec::opaque("SSR Transparent Surface Pipeline")
    }
}

/// `fs_gizmo` writes the authored colour unfogged, as Unity draws its gizmos.
pub(super) fn line_spec() -> PipelineSpec {
    PipelineSpec {
        facing: Facing::Both,
        fragment: "fs_gizmo",
        topology: wgpu::PrimitiveTopology::LineList,
        blend: wgpu::BlendState::ALPHA_BLENDING,
        depth_write_enabled: false,
        depth_compare: crate::gpu::depth::NEARER_OR_EQUAL,
        ..PipelineSpec::opaque("Line Debug Pipeline")
    }
}

pub(super) fn vertex_line_spec() -> PipelineSpec {
    PipelineSpec {
        label: "Vertex Colour Line Pipeline",
        vertex: "vs_gizmo_vertex",
        fragment: "fs_gizmo_vertex",
        ..line_spec()
    }
}

/// Never blended: the nearest surface's motion wins.
pub(crate) fn velocity_target(write_mask: wgpu::ColorWrites) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: crate::motion::VELOCITY_FORMAT,
        blend: None,
        write_mask,
    }
}

/// Indexed as the fragment outputs' `@location`s: colour, then velocity, SSR surfaces or reactive.
fn targets(
    format: wgpu::TextureFormat,
    spec: &PipelineSpec,
) -> Vec<Option<wgpu::ColorTargetState>> {
    let colour = wgpu::ColorTargetState {
        format,
        blend: Some(spec.blend),
        write_mask: wgpu::ColorWrites::ALL,
    };
    let surface = Some(surface_target_state(wgpu::ColorWrites::ALL));
    let surfaces = if spec.surface {
        vec![surface.clone(), surface]
    } else {
        Vec::new()
    };
    let reactive = spec.reactive.then(reactive::target_state);
    std::iter::once(Some(colour))
        .chain(spec.velocity.map(|m| Some(velocity_target(m))))
        .chain(surfaces)
        .chain(reactive.map(Some))
        .collect()
}

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
            targets: &targets(format, spec),
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
