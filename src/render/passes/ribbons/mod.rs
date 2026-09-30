//! src/render/passes/ribbons/ — the Trail and Line ribbon pass (#441).
//!
//! Draws every visible `TrailComponent`'s recorded points and `LineComponent`'s
//! points as camera-facing strips into the HDR target, after the transparent
//! solids and before the particles, so bloom and tonemap apply to an additive
//! tracer. Ribbons are transparent effects: depth-tested against the scene but
//! never writing depth, sorted back-to-front by the shared `draw::sort` key (the
//! one transparent solids and particle emitters use), unlit, fogged in the
//! shader, and absent from the SSAO prepass and the shadow casters — a ribbon
//! takes no AO and casts no shadow.
//!
//! `strip` turns a centre line into geometry (pure CPU); `draw` gathers the
//! scene's ribbons each frame and records the pass. The sim owns a trail's
//! points; this module only reads them.

pub(crate) mod draw;
pub(crate) mod strip;

use crate::components::ParticleBlend;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::gpu::uniforms::FogUniform;
use crate::render::postfx::HDR_FORMAT;
use crate::render::{GpuTexture, Renderer};
use strip::RibbonVertex;

/// Globals uniform: view-projection, the camera position the strips turn to face
/// and fog from, and the scene fog (#437).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RibbonGlobals {
    pub(crate) view_proj: [f32; 16],
    pub(crate) cam_pos: [f32; 4],
    pub(crate) fog: FogUniform,
}

/// Every GPU resource for the ribbon pass. One per `Renderer`.
pub struct RibbonRenderer {
    alpha_pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    pub(crate) globals_buffer: wgpu::Buffer,
    pub(crate) globals_bind_group: wgpu::BindGroup,
    /// What a ribbon with no texture samples, so its colour is its style alone.
    pub(crate) white: GpuTexture,
}

impl RibbonRenderer {
    /// Build the pass: the globals bind group, the white fallback texture and the
    /// two blend pipelines, reusing the renderer's `texture_layout` (group 1).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_layout: &wgpu::BindGroupLayout,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let shader = registry.load(device, "ribbons.wgsl", "Ribbon Shader");
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ribbon Globals Layout"),
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
        });
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Ribbon Globals"),
            size: std::mem::size_of::<RibbonGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ribbon Globals Bind Group"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ribbon Pipeline Layout"),
            bind_group_layouts: &[&globals_layout, texture_layout],
            push_constant_ranges: &[],
        });
        let pipeline = |blend, entry| Self::pipeline(device, &shader, &layout, (blend, entry));
        Self {
            alpha_pipeline: pipeline(wgpu::BlendState::ALPHA_BLENDING, "fs_alpha"),
            additive_pipeline: pipeline(super::particles::additive_blend(), "fs_additive"),
            globals_buffer,
            globals_bind_group,
            white: Renderer::create_white_texture(device, queue, texture_layout),
        }
    }

    fn pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        (blend, fragment_entry): (wgpu::BlendState, &str),
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ribbon Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[RibbonVertex::desc()],
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
            // Both faces: which way a strip winds depends on the view.
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            // Occluded by solids, but never writing depth: overlapping
            // translucent ribbons all blend.
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
