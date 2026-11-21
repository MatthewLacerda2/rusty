//! Not sorted inside an emitter: additive sums in any order, alpha draws in slot order.
//! Compute and vertex-stage storage are required of the adapter; the cap's buffers fit wgpu's default limits.

pub(crate) mod emitter;
pub(crate) mod record;
#[cfg(test)]
mod twin;

use crate::gpu::groups;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use std::collections::{HashMap, HashSet};

use emitter::{EmitterBuffers, Work};
use record::GpuEmitter;

use crate::gpu::layout_entries::{storage_entry, uniform_entry};
use crate::gpu::pipelines::LazyCompute;
use crate::gpu::shaders::ShaderRegistry;
use crate::passes::particles::instance::emitter_light;
use crate::timing::GpuPass;
use crate::Renderer;
use rusty_components::particle::ParticleEmitterComponent;
use rusty_scene::{Scene, SceneId};

const IDLE_RENDERS: u32 = 600;

pub(crate) struct GpuParticles {
    step_layout: wgpu::BindGroupLayout,
    pub(crate) draw_layout: wgpu::BindGroupLayout,
    birth: LazyCompute,
    step: LazyCompute,
    pub(crate) emitters: HashMap<(SceneId, u32), EmitterBuffers>,
}

impl GpuParticles {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let (step_layout, draw_layout) = layouts(device);
        let shader = registry.load(device, "particles_step.wgsl", "GPU Particle Step Shader");
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GPU Particle Step Pipeline Layout"),
            bind_group_layouts: &groups::draw_only(&step_layout),
            immediate_size: 0,
        });
        let pipeline = |entry: &'static str| {
            let (shader, layout) = (shader.clone(), layout.clone());
            LazyCompute::new(device, move |device| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&layout),
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
        };
        Self {
            step_layout,
            draw_layout,
            birth: pipeline("cs_birth"),
            step: pipeline("cs_step"),
            emitters: HashMap::new(),
        }
    }

    pub(crate) fn lazy_computes(&self) -> [&LazyCompute; 2] {
        [&self.birth, &self.step]
    }
}

impl Renderer {
    pub(crate) fn step_gpu_particles(&mut self, scene: &Scene) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GPU Particle Step Encoder"),
            });
        let mut owed: Vec<((SceneId, u32), Work)> = Vec::new();
        let gpu = &mut self.particle_renderer.gpu;
        for e in gpu.emitters.values_mut() {
            e.idle += 1;
        }
        let mut kept = HashSet::new();
        for id in scene.world.ids_with_particles() {
            let Some(emitter) = scene.world.particles(id).filter(|e| e.on_gpu()) else {
                continue;
            };
            let key = (scene.id(), id);
            kept.insert(id);
            let sim = &emitter.runtime.gpu;
            if !emitter.runtime.initialized {
                gpu.emitters.remove(&key);
                continue;
            }
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let uniform = uniform_of(scene, id, &emitter);
            let (device, queue) = (&self.device, &self.queue);
            let wants = emitter::wants(&emitter);
            let buffers = gpu
                .emitters
                .entry(key)
                .or_insert_with(|| EmitterBuffers::new(device, wants.1));
            buffers.idle = 0;
            let work = buffers.upload((device, queue, &mut encoder), sim, uniform, wants);
            owed.push((key, work));
        }
        let this = scene.id();
        gpu.emitters
            .retain(|k, e| (k.0 != this || kept.contains(&k.1)) && e.idle < IDLE_RENDERS);
        self.encode_gpu_steps(encoder, &owed);
    }

    fn encode_gpu_steps(
        &mut self,
        mut encoder: wgpu::CommandEncoder,
        owed: &[((SceneId, u32), Work)],
    ) {
        let dispatches = owed.iter().any(|(_, w)| w.births > 0 || w.step.is_some());
        if !dispatches && !owed.iter().any(|(_, w)| w.encoded) {
            return;
        }
        let pr = &mut self.particle_renderer;
        let (gpu, globals) = (&mut pr.gpu, &pr.globals_buffer);
        for (key, _) in owed {
            if let Some(e) = gpu.emitters.get_mut(key) {
                if e.groups.is_none() {
                    e.groups = Some(groups(
                        &self.device,
                        (&gpu.step_layout, &gpu.draw_layout),
                        e,
                        globals,
                    ));
                }
            }
        }
        let gpu = &self.particle_renderer.gpu;
        if dispatches {
            let mut pass = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("GPU Particle Step"),
                timestamp_writes: self.gpu_timer.compute_writes(GpuPass::Particles),
            });
            for (key, work) in owed {
                let Some((group, _)) = gpu.emitters.get(key).and_then(|e| e.groups.as_ref()) else {
                    continue;
                };
                pass.set_bind_group(groups::DRAW, group, &[]);
                if work.births > 0 {
                    pass.set_pipeline(gpu.birth.get());
                    pass.dispatch_workgroups(work.births.div_ceil(64), 1, 1);
                }
                if let Some(slots) = work.step {
                    pass.set_pipeline(gpu.step.get());
                    pass.dispatch_workgroups(slots.div_ceil(64), 1, 1);
                }
            }
        }
        self.queue.counted_submit(std::iter::once(encoder.finish()));
    }
}

fn groups(
    device: &wgpu::Device,
    layouts: (&wgpu::BindGroupLayout, &wgpu::BindGroupLayout),
    e: &EmitterBuffers,
    globals: &wgpu::Buffer,
) -> (wgpu::BindGroup, wgpu::BindGroup) {
    let step = device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GPU Particle Step Group"),
        layout: layouts.0,
        entries: &e.step_entries(),
    });
    let [emitter, particles, lut, ..] = e.step_entries();
    let draw = device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GPU Particle Draw Group"),
        layout: layouts.1,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            emitter,
            particles,
            lut,
        ],
    });
    (step, draw)
}

fn layouts(device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
    let step_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GPU Particle Step Layout"),
        entries: &step_layout_entries(),
    });
    let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GPU Particle Draw Layout"),
        entries: &draw_layout_entries(),
    });
    (step_layout, draw_layout)
}

pub(crate) fn step_layout_entries() -> [wgpu::BindGroupLayoutEntry; 5] {
    let compute = wgpu::ShaderStages::COMPUTE;
    let rw = wgpu::BindGroupLayoutEntry {
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        ..storage_entry(2, compute)
    };
    [
        uniform_entry(1, compute),
        rw,
        storage_entry(3, compute),
        storage_entry(4, compute),
        storage_entry(5, compute),
    ]
}

pub(crate) fn draw_layout_entries() -> [wgpu::BindGroupLayoutEntry; 4] {
    let vertex = wgpu::ShaderStages::VERTEX;
    [
        uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
        uniform_entry(1, vertex),
        storage_entry(2, vertex),
        storage_entry(3, vertex),
    ]
}

fn uniform_of(scene: &Scene, id: u32, e: &ParticleEmitterComponent) -> GpuEmitter {
    let frame = e.gpu_space();
    GpuEmitter::new(
        e,
        frame,
        emitter_light(scene, e, scene.world.world_position(id)),
    )
}
