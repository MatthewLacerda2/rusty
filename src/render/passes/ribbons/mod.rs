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
use crate::render::passes::blended::{BlendPipelines, Globals};
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
    pipelines: BlendPipelines,
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
        let globals = Globals::new(
            device,
            "Ribbon",
            std::mem::size_of::<RibbonGlobals>() as u64,
        );
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ribbon Pipeline Layout"),
            bind_group_layouts: &[Some(&globals.layout), Some(texture_layout)],
            immediate_size: 0,
        });
        Self {
            pipelines: BlendPipelines::new(
                device,
                "Ribbon Pipeline",
                (&shader, &layout),
                RibbonVertex::desc(),
            ),
            globals_buffer: globals.buffer,
            globals_bind_group: globals.bind_group,
            white: Renderer::create_white_texture(device, queue, texture_layout),
        }
    }

    pub(crate) fn pipeline_for(&self, blend: ParticleBlend) -> &wgpu::RenderPipeline {
        self.pipelines.pipeline_for(blend)
    }
}
