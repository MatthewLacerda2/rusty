//! crates/render/src/passes/particles/gpu/emitter.rs — one GPU-simulated emitter's
//! buffers on the renderer (#2170), and the per-frame upload that brings them to
//! the sim: the births since the last upload, the `dt` of every step since, the
//! lookup table when the curves changed, the uniform.

use super::record::{bake_lut, GpuBirth, GpuEmitter, GpuParticle, LUT_SIZE};
use crate::gpu::ops::CountedQueue as _;
use rusty_components::particle::{GpuSim, DT_HISTORY};

/// The usages of a buffer the frame writes and a pass reads.
const STORAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::COPY_SRC);

/// A birth's record, with the step and the running time it was born at.
type Fresh = (GpuBirth, (u64, f64));

/// What the frame has to dispatch for one emitter after its upload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
    /// Births to scatter into their slots.
    pub(crate) births: u32,
    /// Slots to step (the extent), when any step is owed.
    pub(crate) step: Option<u32>,
    /// Whether the upload recorded a command (a clear, a copy) the frame must submit.
    pub(crate) encoded: bool,
}

/// One emitter's GPU state: its particle buffer (one slot per particle, sized by
/// `max_particles`), the uploads its step reads, and how far the upload has read
/// its [`GpuSim`].
pub(crate) struct EmitterBuffers {
    /// The sim state's reset these buffers follow; a new one empties them.
    epoch: u64,
    /// The next birth serial to upload.
    cursor: u64,
    /// The step the particles were last moved to and the sim's running time
    /// after it; `None` since a reset.
    done: Option<(u64, f64)>,
    /// The sim's [`GpuSim::emptied`] these buffers last saw.
    emptied: u64,
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
    /// The renders since the sim last had this emitter's buffers drawn.
    pub(crate) idle: u32,
}

/// The bytes of a `count`-slot buffer of `T`s.
fn bytes<T>(count: u32) -> u64 {
    (std::mem::size_of::<T>() as u64 * u64::from(count.max(1))).max(16)
}

/// The births buffer's slots for `count` births: rounded up to a power of two,
/// so it grows rarely.
fn births_slots(count: u32) -> u32 {
    count.next_power_of_two()
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
        size: bytes::<T>(count),
        usage,
        mapped_at_creation: false,
    })
}

impl EmitterBuffers {
    /// Empty buffers for `capacity` slots (wgpu zeroes a new buffer, and an
    /// all-zero slot is empty).
    pub(crate) fn new(device: &wgpu::Device, capacity: u32) -> Self {
        let capacity = capacity.max(1);
        let uniform_usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        Self {
            epoch: 0,
            cursor: 0,
            done: None,
            emptied: 0,
            particles: buffer::<GpuParticle>(device, "GPU Particles", capacity, STORAGE),
            capacity,
            births: buffer::<GpuBirth>(device, "GPU Particle Births", 64, STORAGE),
            births_capacity: 64,
            dts: buffer::<f32>(device, "GPU Particle Steps", DT_HISTORY as u32, STORAGE),
            lut: buffer::<[f32; 4]>(device, "GPU Particle Curves", 2 * LUT_SIZE as u32, STORAGE),
            baked: Vec::new(),
            uniform: buffer::<GpuEmitter>(device, "GPU Particle Emitter", 1, uniform_usage),
            emitter: bytemuck::Zeroable::zeroed(),
            groups: None,
            idle: 0,
        }
    }

    /// Bring the buffers to `sim` for an emitter whose uniform is `emitter` and
    /// whose curves bake to `lut`, needing `wanted` slots: grow, empty on a new epoch or once every
    /// particle has died, upload the new births, the owed steps' `dt`s, the table
    /// when it changed and the uniform. Returns what to dispatch.
    pub(crate) fn upload(
        &mut self,
        (device, queue, encoder): (&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder),
        sim: &GpuSim,
        mut emitter: GpuEmitter,
        (lut, wanted): (Vec<[f32; 4]>, u32),
    ) -> Work {
        let mut encoded = false;
        if self.epoch != sim.epoch() {
            self.reset(encoder, sim.epoch());
            encoded = true;
        } else if self.emptied != sim.emptied() {
            // Every particle died since the last upload, so whatever a slot holds is
            // stale: a birth the upload never saw (born and dead in between) left an
            // older particle in its slot, unstepped while the extent was 0.
            encoder.clear_buffer(&self.particles, 0, None);
            encoded = true;
        }
        self.emptied = sim.emptied();
        encoded |= self.grow(device, encoder, wanted);
        if lut != self.baked {
            queue.counted_write_buffer(&self.lut, 0, bytemuck::cast_slice(&lut));
            self.baked = lut;
        }
        let (births, oldest) = self.new_births(sim);
        let goal = sim.steps();
        let (dts, first, jumped) = self.window(sim, oldest);
        let births: Vec<GpuBirth> = births
            .into_iter()
            .map(|(b, born)| GpuBirth {
                jumped: jumped(born.0, born.1),
                ..b
            })
            .collect();
        let extent = sim.extent().min(self.capacity);
        emitter.gravity[3] = self.done.map_or(0.0, |(step, time)| jumped(step, time));
        emitter.info[1] = extent;
        emitter.info[2] = goal as u32;
        emitter.info[3] = first as u32;
        emitter.counts = [births.len() as u32, dts.len() as u32, 0, 0];
        self.done = Some((goal, sim.time()));
        self.emitter = emitter;
        if !births.is_empty() {
            self.grow_births(device, births.len() as u32);
            queue.counted_write_buffer(&self.births, 0, bytemuck::cast_slice(&births));
        }
        if !dts.is_empty() {
            queue.counted_write_buffer(&self.dts, 0, bytemuck::cast_slice(&dts));
        }
        queue.counted_write_buffer(&self.uniform, 0, bytemuck::bytes_of(&emitter));
        Work {
            births: births.len() as u32,
            step: (!dts.is_empty() && extent > 0).then_some(extent),
            encoded,
        }
    }

    /// The steps to replay one by one, from the older of the last step reached
    /// and `oldest` (the oldest new birth's) and at most [`DT_HISTORY`] of them:
    /// their `dt`s, the first one's number, and what the steps before them took
    /// for a particle that starts after step `step` at running time `time`. That
    /// is the sim's running time, so a jump ages by the time that really passed
    /// (a pause at time scale 0 included), not by a window `dt` times the count.
    fn window(
        &self,
        sim: &GpuSim,
        oldest: Option<u64>,
    ) -> (Vec<f32>, u64, impl Fn(u64, f64) -> f32) {
        let goal = sim.steps();
        let done = self.done.map(|(step, _)| step);
        let from = match (oldest, done) {
            (Some(b), Some(d)) => b.min(d),
            (b, d) => b.or(d).unwrap_or(goal),
        };
        let start = (from + 1).max((goal + 1).saturating_sub(DT_HISTORY as u64));
        let dts: Vec<f32> = (start..=goal).filter_map(|s| sim.dt_of(s)).collect();
        let first = goal + 1 - dts.len() as u64;
        let window_starts = sim.time_at(first - 1).unwrap_or_else(|| sim.time());
        let jumped = move |step: u64, time: f64| {
            if step + 1 < first {
                (window_starts - time) as f32
            } else {
                0.0
            }
        };
        (dts, first, jumped)
    }

    /// The births since the last upload, one per slot (a slot reused within the
    /// upload keeps its newest birth, so the scatter never races), dead ones as
    /// empty slots, none past the buffer, each with the step and running time it
    /// was born at; and the step the oldest live one was born at. Reads only what
    /// is new: the sim finds the first new serial by a binary search.
    fn new_births(&mut self, sim: &GpuSim) -> (Vec<Fresh>, Option<u64>) {
        let mut oldest = None::<u64>;
        let mut records: Vec<Fresh> = sim
            .births_from(self.cursor)
            .filter(|b| b.slot < self.capacity)
            .map(|b| {
                if !b.dead {
                    oldest = Some(oldest.map_or(b.step, |o| o.min(b.step)));
                }
                let record = GpuBirth {
                    particle: GpuParticle::born(b),
                    slot: b.slot,
                    jumped: 0.0,
                    pad: [0; 2],
                };
                (record, (b.step, b.time))
            })
            .collect();
        self.cursor = sim.next_serial();
        // Stable, so a slot's births stay oldest first: the last of each run is kept.
        records.sort_by_key(|r| r.0.slot);
        let mut kept: Vec<Fresh> = Vec::with_capacity(records.len());
        for r in records {
            match kept.last_mut() {
                Some(last) if last.0.slot == r.0.slot => *last = r,
                _ => kept.push(r),
            }
        }
        (kept, oldest)
    }

    /// Empty every slot for the sim reset `epoch`: a GPU clear, nothing built on the CPU.
    fn reset(&mut self, encoder: &mut wgpu::CommandEncoder, epoch: u64) {
        encoder.clear_buffer(&self.particles, 0, None);
        (self.epoch, self.cursor, self.done) = (epoch, 0, None);
    }

    /// Hold at least `slots` particles: a bigger buffer (zeroed, so its new slots
    /// are empty) with the old slots copied in. Whether it recorded the copy.
    fn grow(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        slots: u32,
    ) -> bool {
        if slots <= self.capacity {
            return false;
        }
        let bigger = buffer::<GpuParticle>(device, "GPU Particles", slots, STORAGE);
        let old = u64::from(self.capacity) * std::mem::size_of::<GpuParticle>() as u64;
        encoder.copy_buffer_to_buffer(&self.particles, 0, &bigger, 0, old);
        (self.particles, self.capacity, self.groups) = (bigger, slots, None);
        true
    }

    /// Hold at least `count` births.
    fn grow_births(&mut self, device: &wgpu::Device, count: u32) {
        if count <= self.births_capacity {
            return;
        }
        let size = births_slots(count);
        self.births = buffer::<GpuBirth>(device, "GPU Particle Births", size, STORAGE);
        (self.births_capacity, self.groups) = (size, None);
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
/// for its cap.
pub(crate) fn wants(emitter: &rusty_components::ParticleEmitterComponent) -> (Vec<[f32; 4]>, u32) {
    (bake_lut(emitter), emitter.particle_cap())
}

#[cfg(test)]
#[path = "emitter_tests.rs"]
mod tests;
