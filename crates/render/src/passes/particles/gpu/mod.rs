//! crates/render/src/passes/particles/gpu/ — GPU-simulated emitters (#2170):
//! Niagara's *GPUCompute Sim* on the same emitter, with no node graph.
//!
//! A `Gpu` emitter's live particles are render state. The sim makes its births on
//! the CPU and keeps its exact live count (`components::particle::GpuSim`); once
//! per frame, never per camera, [`Renderer::step_gpu_particles`] uploads what is
//! new of each emitter ([`emitter`]: the births into their slots, the `dt` of every
//! step since the last upload) and runs `particles_step.wgsl`: `cs_birth` scatters
//! the births, `cs_step` moves every particle step by step up to the sim's step.
//! Every camera then draws the same buffer (`particles.wgsl`'s `vs_gpu`, one
//! instance per slot, an empty slot culled), so nothing per particle is rebuilt,
//! sorted or uploaded per camera, whether a stack's or a render texture's.
//!
//! **Not sorted inside an emitter.** Additive sprites sum the same in any order;
//! alpha ones draw in slot order. Emitters still sort back to front among
//! themselves, a `Gpu` one by where it stands (no centroid is known on the CPU).
//!
//! **Compute is a requirement**, not a choice made here: the renderer's other
//! passes already bind storage buffers in the vertex stage and warm compute
//! pipelines (the Hi-Z cull, the fog), so an adapter without them never builds a
//! renderer. `twin` holds the CPU twins the tests hold its shaders to.
//!
//! **Capacity.** The sim caps a `Gpu` emitter at `GPU_MAX_PARTICLES` on every
//! device, and that cap's buffers fit wgpu's default limits, which every device
//! is created with (`setup::limits::device_limits`); `emitter`'s tests hold it.

pub(crate) mod emitter;
pub(crate) mod record;
#[cfg(test)]
mod twin;

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

/// Renders without a draw after which an emitter's buffers are dropped: a scene
/// nobody renders any more (a replaced one) frees its emitters.
const IDLE_RENDERS: u32 = 600;

/// Every GPU-simulated emitter's buffers, and the pipelines that step them.
pub(crate) struct GpuParticles {
    /// The step's group: the emitter, particles (read-write), curves, `dt`s, births.
    step_layout: wgpu::BindGroupLayout,
    /// The draw's group 0: the pass globals, the emitter, particles, curves.
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
            bind_group_layouts: &[Some(&step_layout)],
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

    /// Both compute pipelines, built or not.
    pub(crate) fn lazy_computes(&self) -> [&LazyCompute; 2] {
        [&self.birth, &self.step]
    }
}

impl Renderer {
    /// Bring every visible `Gpu` emitter of `scene` to the sim, once per frame:
    /// upload, then scatter the births and step (see the module docs).
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
                // Not in Play (edit mode, or after Stop): nothing to keep.
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
        // An emitter gone or back on the CPU frees its buffers now; another
        // scene's after `IDLE_RENDERS` renders without it.
        let this = scene.id();
        gpu.emitters
            .retain(|k, e| (k.0 != this || kept.contains(&k.1)) && e.idle < IDLE_RENDERS);
        self.encode_gpu_steps(encoder, &owed);
    }

    /// Record and submit the births' scatter and the step of each emitter in `owed`;
    /// nothing at all when nothing is owed (a scene with no `Gpu` emitter, a
    /// capture face, a frame whose emitters took no step and had no birth).
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
                pass.set_bind_group(0, group, &[]);
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

/// `e`'s step group (`layouts.0`) and draw group 0 (`layouts.1`, with the pass
/// `globals`).
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

/// The step's group layout (the emitter, particles read-write, curves, `dt`s,
/// births) and the draw's group 0 (the pass globals, the emitter, particles, curves).
fn layouts(device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
    let compute = wgpu::ShaderStages::COMPUTE;
    let rw = wgpu::BindGroupLayoutEntry {
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        ..storage_entry(2, compute)
    };
    let step_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GPU Particle Step Layout"),
        entries: &[
            uniform_entry(1, compute),
            rw,
            storage_entry(3, compute),
            storage_entry(4, compute),
            storage_entry(5, compute),
        ],
    });
    let vertex = wgpu::ShaderStages::VERTEX;
    let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GPU Particle Draw Layout"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
            uniform_entry(1, vertex),
            storage_entry(2, vertex),
            storage_entry(3, vertex),
        ],
    });
    (step_layout, draw_layout)
}

/// Entity `id`'s emitter `e` as its uniform: its stored particles under the
/// space they are stored in (`gpu_space`: identity for `World`, the change from
/// its first frame with an inverse to its last for `Local`, so an emitter at
/// scale 0 draws its particles where they last stood, as the `Cpu` path leaves
/// them, and none collapses onto the emitter), lit where it stands.
fn uniform_of(scene: &Scene, id: u32, e: &ParticleEmitterComponent) -> GpuEmitter {
    let frame = e.gpu_space();
    GpuEmitter::new(
        e,
        frame,
        emitter_light(scene, e, scene.world.world_position(id)),
    )
}
