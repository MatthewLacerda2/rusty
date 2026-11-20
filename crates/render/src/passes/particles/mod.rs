//! crates/render/src/passes/particles/ — the particle pass (#13, #440): resources +
//! pipelines here, per-frame orchestration in `draw`, instance building in
//! `instance`, and mesh particles (which join the forward solids) in `mesh`, their
//! loaded meshes cached in `mesh_cache`.
//!
//! Sprite particles draw into the HDR scene target BEFORE the post-FX chain — so
//! bloom/tonemap apply to additive (emissive) sparks. Two pipelines share one
//! shader: alpha-blended (smoke) and additive (sparks/fire). Both read the scene
//! depth as a read-only attachment (occlusion) *and* a texture (soft particles),
//! and bind the renderer's group 0 so lit particles read the forward lighting
//! uniform.
//!
//! The simulation owns particle *state*; this module only reads it — the invariant
//! that keeps headless play renderer-free. A GPU-simulated emitter (`gpu`, #2170)
//! is the one exception by design: its live particles are render state, kept and
//! moved here from the births the sim makes, and the sim never reads them.

mod draw;
pub(crate) use draw::ParticleDraws;
pub(crate) mod gpu;
pub(crate) mod instance;
mod mesh;
mod mesh_cache;
mod runs;
pub(crate) use mesh_cache::CachedMesh;

use std::collections::HashMap;
use std::rc::Rc;

use wgpu::util::DeviceExt;

use crate::gpu::groups::Groups;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::passes::blended::{BlendPipelines, Globals};
use crate::GpuTexture;
use instance::{ParticleGlobals, ParticleInstance};
use rusty_components::particle::ParticleBlend;

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
    /// The blend pipelines of a GPU-simulated emitter's draw (`vs_gpu`, #2170).
    gpu_pipelines: BlendPipelines,
    /// The GPU-simulated emitters' buffers and step (`gpu`).
    pub(crate) gpu: gpu::GpuParticles,
    pub(crate) globals_buffer: wgpu::Buffer,
    pub(crate) globals_bind_group: wgpu::BindGroup,
    pub(crate) index_buffer: wgpu::Buffer,
    /// Mesh-particle specs resolved to their GPU mesh, so a model is read from
    /// disk once, not every frame, yet follows a re-import (`mesh_cache`).
    pub(crate) meshes: HashMap<String, CachedMesh>,
    /// What an emitter naming no texture draws: Unity's soft white disc
    /// (`Renderer::create_soft_disc_texture`, #2052). A texture that fails to
    /// load still draws the renderer's checker.
    pub(crate) default_sprite: Rc<GpuTexture>,
}

impl ParticleRenderer {
    /// Build the pass: a globals bind group + two blend-variant pipelines over
    /// groups globals (0), sprite (1), scene depth (2) and camera + lighting (3);
    /// `default_sprite` is what an emitter with no texture binds as its sprite.
    pub(crate) fn new(
        device: &wgpu::Device,
        layouts: SharedLayouts,
        default_sprite: Rc<GpuTexture>,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let shader = registry.load(device, "particles.wgsl", "Particle Shader");

        let globals = Globals::new(
            device,
            "Particle",
            std::mem::size_of::<ParticleGlobals>() as u64,
        );

        let shared = (&shader, &layouts);
        let instances = ("vs_main", Some(ParticleInstance::desc()));
        let pipelines = blend_pipelines(device, "Particle", shared, &globals.layout, instances);
        let gpu = gpu::GpuParticles::new(device, registry);
        let gpu_pipelines = blend_pipelines(
            device,
            "GPU Particle",
            shared,
            &gpu.draw_layout,
            ("vs_gpu", None),
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
            gpu_pipelines,
            gpu,
            globals_buffer: globals.buffer,
            globals_bind_group: globals.bind_group,
            index_buffer,
            meshes: HashMap::new(),
            default_sprite,
        }
    }

    /// The pipeline for `blend`, into TAA's reactive mask too when `reactive` (#1845).
    pub(crate) fn pipeline_for(
        &self,
        blend: ParticleBlend,
        reactive: bool,
    ) -> &wgpu::RenderPipeline {
        self.pipelines.pipeline_for(blend, reactive)
    }

    /// The pipeline drawing a GPU-simulated emitter's buffer for `blend` (#2170).
    pub(crate) fn gpu_pipeline_for(
        &self,
        blend: ParticleBlend,
        reactive: bool,
    ) -> &wgpu::RenderPipeline {
        self.gpu_pipelines.pipeline_for(blend, reactive)
    }

    /// Every blend pipeline, built or not (#985).
    pub(crate) fn lazy_pipelines(&self) -> Vec<&LazyPipeline> {
        let mut all = self.pipelines.lazy().to_vec();
        all.extend(self.gpu_pipelines.lazy());
        all
    }
}

/// A particle draw's groups by role ([`Groups`]): the renderer's camera + lighting
/// group, the scene `depth`, the `sprite` at the material's index, and `draw` (the
/// pass globals, or a GPU emitter's own) per draw, last.
pub(crate) fn particle_groups<T>(camera_lighting: T, depth: T, sprite: T, draw: T) -> Groups<T> {
    Groups {
        global: camera_lighting,
        scene: Some(depth),
        material: Some(sprite),
        draw,
    }
}

/// The blend pipelines of `shader`'s vertex entry point `vertex.0` (drawing the
/// `vertex.1` buffer, if any) over [`particle_groups`] with `draw`.
fn blend_pipelines(
    device: &wgpu::Device,
    label: &'static str,
    (shader, layouts): (&wgpu::ShaderModule, &SharedLayouts),
    draw: &wgpu::BindGroupLayout,
    vertex: (&'static str, Option<wgpu::VertexBufferLayout<'static>>),
) -> BlendPipelines {
    let groups = particle_groups(
        layouts.camera_lighting,
        layouts.depth,
        layouts.texture,
        draw,
    );
    let layout = groups.layout(device, label);
    BlendPipelines::new(device, label, (shader, &layout), vertex)
}
