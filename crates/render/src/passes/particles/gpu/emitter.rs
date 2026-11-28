//! An all-zero slot is empty (wgpu zeroes new buffers). A slot reused within an upload keeps its
//! newest birth, so the scatter never races. Steps older than the dt window are one jump by the
//! sim's running time, so a pause at time scale 0 ages correctly.

use super::record::{bake_lut, GpuBirth, GpuEmitter, GpuParticle, LUT_SIZE};
use crate::gpu::ops::CountedQueue as _;
use glam::Vec3;
use rusty_components::particle::{GpuSim, DT_HISTORY};

const STORAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::COPY_SRC);

type Fresh = (GpuBirth, (u64, f64));

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
    pub(crate) births: u32,
    pub(crate) step: Option<u32>,
    /// A clear or copy was recorded: the frame must submit, dispatch or not.
    pub(crate) encoded: bool,
}

pub(crate) struct EmitterBuffers {
    epoch: u64,
    cursor: u64,
    /// The step last moved to, the running seconds after it; `None` since a reset.
    done: Option<(u64, f64)>,
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
    pub(crate) groups: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
    pub(crate) idle: u32,
    moved: glam::Vec3,
}

fn bytes<T>(count: u32) -> u64 {
    (std::mem::size_of::<T>() as u64 * u64::from(count.max(1))).max(16)
}

fn births_slots(count: u32) -> u32 {
    count.next_power_of_two()
}

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
            moved: glam::Vec3::ZERO,
        }
    }

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
        // All died since the last upload: a birth born and dead unseen left a stale particle.
        } else if self.emptied != sim.emptied() {
            encoder.clear_buffer(&self.particles, 0, None);
            encoded = true;
        }
        if encoded {
            self.moved = sim.moved();
        }
        self.emptied = sim.emptied();
        let pending = sim.moved() - self.moved;
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
            .map(|(mut b, born)| {
                b.particle.position = (Vec3::from(b.particle.position) - pending).to_array();
                GpuBirth {
                    jumped: jumped(born.0, born.1),
                    ..b
                }
            })
            .collect();
        let extent = sim.extent().min(self.capacity);
        emitter.gravity[3] = self.done.map_or(0.0, |(step, time)| jumped(step, time));
        emitter.info[1] = extent;
        emitter.info[2] = goal as u32;
        emitter.info[3] = first as u32;
        emitter.counts = [births.len() as u32, dts.len() as u32, 0, 0];
        let step = (!dts.is_empty() && extent > 0).then_some(extent);
        if step.is_some() {
            emitter.shift = pending.extend(0.0).to_array();
            self.moved = sim.moved();
        }
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
            step,
            encoded,
        }
    }

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
        // Stable: a slot's births stay oldest first.
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

    fn reset(&mut self, encoder: &mut wgpu::CommandEncoder, epoch: u64) {
        encoder.clear_buffer(&self.particles, 0, None);
        (self.epoch, self.cursor, self.done) = (epoch, 0, None);
    }

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

    fn grow_births(&mut self, device: &wgpu::Device, count: u32) {
        if count <= self.births_capacity {
            return;
        }
        let size = births_slots(count);
        self.births = buffer::<GpuBirth>(device, "GPU Particle Births", size, STORAGE);
        (self.births_capacity, self.groups) = (size, None);
    }

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

pub(crate) fn wants(emitter: &rusty_components::ParticleEmitterComponent) -> (Vec<[f32; 4]>, u32) {
    (bake_lut(emitter), emitter.particle_cap())
}

#[cfg(test)]
#[path = "emitter_tests.rs"]
mod tests;
