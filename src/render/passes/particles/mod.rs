//! src/render/passes/particles/ — the particle pass (#13, #440): resources +
//! pipelines here, per-frame orchestration in `draw`, instance building in
//! `instance`, and mesh particles (which join the forward solids) in `mesh`.
//!
//! Sprite particles draw into the HDR scene target BEFORE the post-FX chain — so
//! bloom/tonemap apply to additive (emissive) sparks. Two pipelines share one
//! shader: alpha-blended (smoke) and additive (sparks/fire). Both read the scene
//! depth as a read-only attachment (occlusion) *and* a texture (soft particles),
//! and bind the renderer's group 0 so lit particles read the forward lighting
//! uniform.
//!
//! The simulation owns particle *state*; this module only reads it — the invariant
//! that keeps headless play renderer-free.

mod draw;
pub(crate) use draw::ParticleDraws;
pub(crate) mod instance;
mod mesh;

use std::collections::HashMap;

use wgpu::util::DeviceExt;

use crate::components::particle::ParticleBlend;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::HDR_FORMAT;
use crate::render::MeshId;
use instance::{ParticleGlobals, ParticleInstance};

/// The layouts the particle pipelines share with other passes: the single-texture
/// sprite layout, the scene-depth layout (the decal pass's), and the renderer's
/// camera + lighting group 0.
pub(crate) struct SharedLayouts<'a> {
    pub texture: &'a wgpu::BindGroupLayout,
    pub depth: &'a wgpu::BindGroupLayout,
    pub camera_lighting: &'a wgpu::BindGroupLayout,
}

/// Owns every GPU resource for the particle pass. One per `Renderer`.
pub struct ParticleRenderer {
    alpha_pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    pub(crate) globals_buffer: wgpu::Buffer,
    pub(crate) globals_bind_group: wgpu::BindGroup,
    pub(crate) index_buffer: wgpu::Buffer,
    /// Mesh-particle mesh names resolved to their GPU mesh (`None`: failed to load),
    /// so a model is read from disk once, not every frame.
    pub(crate) meshes: HashMap<String, Option<MeshId>>,
}

impl ParticleRenderer {
    /// Build the pass: a globals bind group + two blend-variant pipelines over
    /// groups globals (0), sprite (1), scene depth (2) and camera + lighting (3).
    pub(crate) fn new(
        device: &wgpu::Device,
        layouts: SharedLayouts,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let shader = registry.load(device, "particles.wgsl", "Particle Shader");

        let globals_layout = Self::globals_layout(device);
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Globals"),
            size: std::mem::size_of::<ParticleGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Globals Bind Group"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[
                &globals_layout,
                layouts.texture,
                layouts.depth,
                layouts.camera_lighting,
            ],
            push_constant_ranges: &[],
        });

        let alpha_pipeline = Self::pipeline(
            device,
            &shader,
            &layout,
            (wgpu::BlendState::ALPHA_BLENDING, "fs_alpha"),
        );
        let additive_pipeline =
            Self::pipeline(device, &shader, &layout, (additive_blend(), "fs_additive"));

        // Two triangles (a quad) shared by every billboard instance.
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Quad Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            alpha_pipeline,
            additive_pipeline,
            globals_buffer,
            globals_bind_group,
            index_buffer,
            meshes: HashMap::new(),
        }
    }

    fn globals_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Globals Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        (blend, fragment_entry): (wgpu::BlendState, &str),
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[ParticleInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: fragment_entry,
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            // Depth-test against the scene depth (so particles are occluded by
            // solids) but DON'T write depth — overlapping transparent sprites must
            // all blend, not z-fight. Read-only, so the same depth can also be bound
            // as the soft-particle texture.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    pub(crate) fn pipeline_for(&self, blend: ParticleBlend) -> &wgpu::RenderPipeline {
        match blend {
            ParticleBlend::Alpha => &self.alpha_pipeline,
            ParticleBlend::Additive => &self.additive_pipeline,
        }
    }
}

/// Additive blend (src + dst), keeping dst alpha. Sparks/fire brighten the HDR
/// target so the bloom pass picks them up.
pub(crate) fn additive_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    }
}
