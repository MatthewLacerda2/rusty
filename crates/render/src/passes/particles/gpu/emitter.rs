//! crates/render/src/passes/particles/gpu/emitter.rs — one GPU-simulated emitter's
//! buffers on the renderer (#2170), and the per-frame upload that brings them to
//! the sim: the births since the last upload, the `dt` of every step since, the
//! lookup table when the curves changed, the uniform.

use glam::Vec3;

use super::record::{advance, bake_lut, GpuBirth, GpuEmitter, GpuParticle, LUT_SIZE};
use rusty_components::particle::{GpuSim, DT_HISTORY};

/// The usages of a buffer the frame writes and a pass reads.
const STORAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::COPY_SRC);

/// What the frame has to dispatch for one emitter after its upload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
    /// Births to scatter into their slots.
    pub(crate) births: u32,
    /// Slots to step (the extent), when any step is owed.
    pub(crate) step: Option<u32>,
}

/// One emitter's GPU state: its particle buffer (one slot per particle, sized by
/// `max_particles`), the uploads its step reads, and how far the upload has read
/// its [`GpuSim`].
pub(crate) struct EmitterBuffers {
    /// The sim state's reset these buffers follow; a new one empties them.
    epoch: u64,
    /// The next birth serial to upload.
    cursor: u64,
    /// The step the particles were last moved to; `None` since a reset.
    done: Option<u64>,
    pub(crate) particles: wgpu::Buffer,
    pub(crate) capacity: u32,
    births: wgpu::Buffer,
    births_capacity: u32,
    dts: wgpu::Buffer,
    pub(crate) lut: wgpu::Buffer,
    baked: Vec<[f32; 4]>,
    pub(crate) uniform: wgpu::Buffer,
    pub(crate) emitter: GpuEmitter,
    /// The bind groups, rebuilt when a buffer they hold grew.
    pub(crate) groups: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
    /// The fallback's particles, on the CPU, for an adapter with no compute.
    pub(crate) mirror: Option<Vec<GpuParticle>>,
    /// The renders since the sim last had this emitter's buffers drawn.
    pub(crate) idle: u32,
}

/// A `count`-slot buffer of `T`s labelled `label`.
fn buffer<T>(
    device: &wgpu::Device,
    label: &str,
    count: u32,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (std::mem::size_of::<T>() as u64 * u64::from(count.max(1))).max(16),
        usage,
        mapped_at_creation: false,
    })
}

impl EmitterBuffers {
    /// Empty buffers for `capacity` slots; `mirror` keeps them on the CPU instead.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        capacity: u32,
        mirror: bool,
    ) -> Self {
        let capacity = capacity.max(1);
        let particles = buffer::<GpuParticle>(device, "GPU Particles", capacity, STORAGE);
        queue.write_buffer(
            &particles,
            0,
            bytemuck::cast_slice(&vec![GpuParticle::EMPTY; capacity as usize]),
        );
        let uniform_usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        Self {
            epoch: 0,
            cursor: 0,
            done: None,
            particles,
            capacity,
            births: buffer::<GpuBirth>(device, "GPU Particle Births", 64, STORAGE),
            births_capacity: 64,
            dts: buffer::<f32>(device, "GPU Particle Steps", DT_HISTORY as u32, STORAGE),
            lut: buffer::<[f32; 4]>(device, "GPU Particle Curves", 2 * LUT_SIZE as u32, STORAGE),
            baked: Vec::new(),
            uniform: buffer::<GpuEmitter>(device, "GPU Particle Emitter", 1, uniform_usage),
            emitter: bytemuck::Zeroable::zeroed(),
            groups: None,
            mirror: mirror.then(Vec::new),
            idle: 0,
        }
    }

    /// Bring the buffers to `sim` for an emitter whose uniform is `emitter` and
    /// whose curves bake to `lut`, needing `wanted` slots: grow, empty on a new
    /// epoch, upload the new births, the owed steps' `dt`s, the table when it
    /// changed and the uniform. Returns what to dispatch.
    pub(crate) fn upload(
        &mut self,
        (device, queue, encoder): (&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder),
        sim: &GpuSim,
        mut emitter: GpuEmitter,
        (lut, wanted): (Vec<[f32; 4]>, u32),
    ) -> Work {
        if self.epoch != sim.epoch() {
            self.reset(queue, sim.epoch());
        }
        self.grow(device, queue, encoder, wanted.max(sim.extent()));
        if lut != self.baked {
            queue.write_buffer(&self.lut, 0, bytemuck::cast_slice(&lut));
            self.baked = lut;
        }
        let (births, oldest) = self.new_births(sim);
        // Moved from the older of the last step reached and the oldest new birth's.
        let goal = sim.steps();
        let from = match (oldest, self.done) {
            (Some(b), Some(d)) => b.min(d),
            (b, d) => b.or(d).unwrap_or(goal),
        };
        let start = (from + 1).max((goal + 1).saturating_sub(DT_HISTORY as u64));
        let dts: Vec<f32> = (start..=goal).filter_map(|s| sim.dt_of(s)).collect();
        let first = goal + 1 - dts.len() as u64;
        emitter.info[1] = sim.extent();
        emitter.info[2] = goal as u32;
        emitter.info[3] = first as u32;
        emitter.counts = [births.len() as u32, dts.len() as u32, 0, 0];
        self.done = Some(goal);
        self.emitter = emitter;
        if let Some(mirror) = &mut self.mirror {
            mirror.resize(self.capacity as usize, GpuParticle::EMPTY);
            for b in &births {
                mirror[b.slot as usize] = b.particle;
            }
            let gravity = Vec3::from_slice(&emitter.gravity[..3]);
            let dt_of = |s: u32| {
                // As the shader: a step older than the window takes its oldest `dt`.
                let at = s.wrapping_sub(first as u32);
                let at = if at >= 0x8000_0000 { 0 } else { at as usize };
                dts[at.min(dts.len() - 1)]
            };
            if !dts.is_empty() {
                for p in mirror.iter_mut().take(sim.extent() as usize) {
                    advance(p, goal as u32, gravity, &self.baked, dt_of);
                }
            }
            return Work::default();
        }
        if !births.is_empty() {
            self.grow_births(device, births.len() as u32);
            queue.write_buffer(&self.births, 0, bytemuck::cast_slice(&births));
        }
        if !dts.is_empty() {
            queue.write_buffer(&self.dts, 0, bytemuck::cast_slice(&dts));
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&emitter));
        Work {
            births: births.len() as u32,
            step: (!dts.is_empty() && sim.extent() > 0).then_some(sim.extent()),
        }
    }

    /// The births since the last upload, one per slot (a slot reused within the
    /// upload keeps its newest birth, so the scatter never races), dead ones as
    /// empty slots; and the step the oldest live one was born at.
    fn new_births(&mut self, sim: &GpuSim) -> (Vec<GpuBirth>, Option<u64>) {
        let mut by_slot: Vec<GpuBirth> = Vec::new();
        let mut seen = std::collections::HashMap::new();
        let mut oldest = None::<u64>;
        for b in sim.births().filter(|b| b.serial >= self.cursor) {
            if !b.dead {
                oldest = Some(oldest.map_or(b.step, |o| o.min(b.step)));
            }
            let record = GpuBirth {
                particle: GpuParticle::born(b),
                slot: b.slot,
                pad: [0; 3],
            };
            match seen.get(&b.slot) {
                Some(&at) => by_slot[at] = record,
                None => {
                    seen.insert(b.slot, by_slot.len());
                    by_slot.push(record);
                }
            }
        }
        self.cursor = sim.next_serial();
        (by_slot, oldest)
    }

    /// Empty every slot for the sim reset `epoch`.
    fn reset(&mut self, queue: &wgpu::Queue, epoch: u64) {
        let empty = vec![GpuParticle::EMPTY; self.capacity as usize];
        queue.write_buffer(&self.particles, 0, bytemuck::cast_slice(&empty));
        if let Some(mirror) = &mut self.mirror {
            mirror.clear();
        }
        (self.epoch, self.cursor, self.done) = (epoch, 0, None);
    }

    /// Hold at least `slots` particles: a bigger buffer, the old slots copied in
    /// and the new ones empty.
    fn grow(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        slots: u32,
    ) {
        if slots <= self.capacity {
            return;
        }
        let bigger = buffer::<GpuParticle>(device, "GPU Particles", slots, STORAGE);
        let tail = vec![GpuParticle::EMPTY; (slots - self.capacity) as usize];
        let old = u64::from(self.capacity) * std::mem::size_of::<GpuParticle>() as u64;
        queue.write_buffer(&bigger, old, bytemuck::cast_slice(&tail));
        encoder.copy_buffer_to_buffer(&self.particles, 0, &bigger, 0, old);
        (self.particles, self.capacity, self.groups) = (bigger, slots, None);
    }

    /// Hold at least `count` births.
    fn grow_births(&mut self, device: &wgpu::Device, count: u32) {
        if count <= self.births_capacity {
            return;
        }
        let size = count.next_power_of_two();
        self.births = buffer::<GpuBirth>(device, "GPU Particle Births", size, STORAGE);
        (self.births_capacity, self.groups) = (size, None);
    }

    /// The lookup table last uploaded.
    pub(crate) fn baked(&self) -> &[[f32; 4]] {
        &self.baked
    }

    /// The buffers the step's group binds, in binding order 1..=5.
    pub(crate) fn step_entries(&self) -> [wgpu::BindGroupEntry<'_>; 5] {
        let buffers = [
            &self.uniform,
            &self.particles,
            &self.lut,
            &self.dts,
            &self.births,
        ];
        let mut binding = 0;
        buffers.map(|buffer| {
            binding += 1;
            wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }
        })
    }
}

/// The lookup table and the slots an emitter wants: its curves baked, and room
/// for `max_particles`.
pub(crate) fn wants(emitter: &rusty_components::ParticleEmitterComponent) -> (Vec<[f32; 4]>, u32) {
    (bake_lut(emitter), emitter.max_particles)
}
