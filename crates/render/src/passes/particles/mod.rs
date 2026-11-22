//! Sprite particles draw into the HDR scene target before post-FX, so bloom and tonemap apply to
//! additive sparks. Scene depth is a read-only attachment and a texture (soft particles).
//! The sim owns particle state; the renderer only reads it, keeping headless play renderer-free.
//! GPU emitters are the exception: their live particles are render state.

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

pub(crate) struct SharedLayouts<'a> {
    pub texture: &'a wgpu::BindGroupLayout,
    pub depth: &'a wgpu::BindGroupLayout,
    pub camera_lighting: &'a wgpu::BindGroupLayout,
}

pub struct ParticleRenderer {
    pipelines: BlendPipelines,
    gpu_pipelines: BlendPipelines,
    pub(crate) gpu: gpu::GpuParticles,
    pub(crate) globals_buffer: wgpu::Buffer,
    pub(crate) globals_bind_group: wgpu::BindGroup,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) meshes: HashMap<String, CachedMesh>,
    pub(crate) default_sprite: Rc<GpuTexture>,
}

impl ParticleRenderer {
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

    pub(crate) fn pipeline_for(
        &self,
        blend: ParticleBlend,
        reactive: bool,
    ) -> &wgpu::RenderPipeline {
        self.pipelines.pipeline_for(blend, reactive)
    }

    pub(crate) fn gpu_pipeline_for(
        &self,
        blend: ParticleBlend,
        reactive: bool,
    ) -> &wgpu::RenderPipeline {
        self.gpu_pipelines.pipeline_for(blend, reactive)
    }

    pub(crate) fn lazy_pipelines(&self) -> Vec<&LazyPipeline> {
        let mut all = self.pipelines.lazy().to_vec();
        all.extend(self.gpu_pipelines.lazy());
        all
    }
}

fn blend_pipelines(
    device: &wgpu::Device,
    label: &'static str,
    (shader, layouts): (&wgpu::ShaderModule, &SharedLayouts),
    draw: &wgpu::BindGroupLayout,
    vertex: (&'static str, Option<wgpu::VertexBufferLayout<'static>>),
) -> BlendPipelines {
    let groups = Groups {
        global: layouts.camera_lighting,
        scene: Some(layouts.depth),
        material: Some(layouts.texture),
        draw,
    };
    let layout = groups.layout(device, label);
    BlendPipelines::new(device, label, (shader, &layout), vertex)
}
