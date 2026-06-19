//! src/render/particles.rs — billboard particle pass: resources + pipelines.
//!
//! Owns the GPU resources for drawing each emitter's live `Vec<Particle>` (from the
//! decoupled sim in `app::particles`) as camera-facing textured quads in the HDR
//! scene target, BEFORE the post-FX chain — so bloom/tonemap apply to additive
//! (emissive) sparks. The per-frame draw orchestration lives in `particles_draw`.
//!
//! Two pipelines share one shader: alpha-blended (smoke) and additive (sparks/fire).
//! The simulation owns particle *state*; this module only reads it — the invariant
//! that keeps headless play renderer-free.

use wgpu::util::DeviceExt;

use super::postfx::HDR_FORMAT;
use super::shaders::ShaderRegistry;
use crate::components::particle::ParticleBlend;

/// Per-particle instance data uploaded to the GPU (matches `InstanceInput`).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ParticleInstance {
    pub(super) center: [f32; 3],
    pub(super) size: f32,
    pub(super) color: [f32; 4],
}

impl ParticleInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3, // center
        1 => Float32,   // size
        2 => Float32x4, // color
    ];

    pub(super) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Globals uniform: view-projection + camera right/up for billboarding.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ParticleGlobals {
    pub(super) view_proj: [f32; 16],
    pub(super) cam_right: [f32; 4],
    pub(super) cam_up: [f32; 4],
}

/// Owns every GPU resource for the particle pass. One per `Renderer`.
pub struct ParticleRenderer {
    alpha_pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    pub(super) globals_buffer: wgpu::Buffer,
    pub(super) globals_bind_group: wgpu::BindGroup,
    pub(super) index_buffer: wgpu::Buffer,
}

impl ParticleRenderer {
    /// Build the pass: a globals bind group + two blend-variant pipelines that
    /// reuse the renderer's `texture_layout` for the sprite (group 1).
    pub fn new(
        device: &wgpu::Device,
        texture_layout: &wgpu::BindGroupLayout,
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
            bind_group_layouts: &[&globals_layout, texture_layout],
            push_constant_ranges: &[],
        });

        let alpha_pipeline =
            Self::pipeline(device, &shader, &layout, wgpu::BlendState::ALPHA_BLENDING);
        let additive_pipeline = Self::pipeline(device, &shader, &layout, additive_blend());

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
        }
    }

    fn globals_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Globals Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
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
        blend: wgpu::BlendState,
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
                entry_point: "fs_main",
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
            // all blend, not z-fight.
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

    pub(super) fn pipeline_for(&self, blend: ParticleBlend) -> &wgpu::RenderPipeline {
        match blend {
            ParticleBlend::Alpha => &self.alpha_pipeline,
            ParticleBlend::Additive => &self.additive_pipeline,
        }
    }
}

/// Additive blend (src + dst), keeping dst alpha. Sparks/fire brighten the HDR
/// target so the bloom pass picks them up.
fn additive_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    }
}
