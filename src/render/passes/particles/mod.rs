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
use crate::render::passes::blended::{BlendPipelines, Globals};
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
    pipelines: BlendPipelines,
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

        let globals = Globals::new(
            device,
            "Particle",
            std::mem::size_of::<ParticleGlobals>() as u64,
        );

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[
                Some(&globals.layout),
                Some(layouts.texture),
                Some(layouts.depth),
                Some(layouts.camera_lighting),
            ],
            immediate_size: 0,
        });

        let pipelines = BlendPipelines::new(
            device,
            "Particle Pipeline",
            (&shader, &layout),
            ParticleInstance::desc(),
        );

        // Two triangles (a quad) shared by every billboard instance.
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Quad Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipelines,
            globals_buffer: globals.buffer,
            globals_bind_group: globals.bind_group,
            index_buffer,
            meshes: HashMap::new(),
        }
    }

    pub(crate) fn pipeline_for(&self, blend: ParticleBlend) -> &wgpu::RenderPipeline {
        self.pipelines.pipeline_for(blend)
    }
}
