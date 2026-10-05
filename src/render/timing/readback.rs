//! The timer's readback ring: a few mappable buffers, each holding one frame's
//! resolved timestamps until the CPU reads them. A buffer is free, mapping (its
//! frame's copy is in flight), or mapped (ready to read); the ring hands out free
//! ones and never waits for a busy one.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use super::GpuPass;

/// Frames that may be in flight at once before one goes untimed.
const SLOTS: usize = 3;

const FREE: u8 = 0;
const MAPPING: u8 = 1;
const MAPPED: u8 = 2;

/// One frame's readback buffer.
struct Slot {
    buffer: wgpu::Buffer,
    /// The frame's timed passes, one per query pair.
    passes: Vec<GpuPass>,
    /// The frame's place in submission order, so the newest finished one wins.
    frame: u64,
    /// `FREE` / `MAPPING` / `MAPPED`, set by the map callback from wgpu's thread.
    state: Arc<AtomicU8>,
}

pub(super) struct Ring {
    /// The query set resolves here first: a resolve target cannot be mapped.
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    submitted: u64,
}

impl Ring {
    /// A ring whose buffers hold `queries` timestamps each.
    pub fn new(device: &wgpu::Device, queries: u64) -> Self {
        let size = queries * std::mem::size_of::<u64>() as u64;
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let resolve = buffer(
            "GPU Timestamp Resolve",
            wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        );
        let slots = (0..SLOTS)
            .map(|_| Slot {
                buffer: buffer(
                    "GPU Timestamp Readback",
                    wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                ),
                passes: Vec::new(),
                frame: 0,
                state: Arc::new(AtomicU8::new(FREE)),
            })
            .collect();
        Self {
            resolve,
            slots,
            submitted: 0,
        }
    }

    /// Resolve the first `passes.len()` query pairs of `set` into a free slot and
    /// start mapping it. No free slot: the frame goes untimed.
    pub fn submit(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        set: &wgpu::QuerySet,
        passes: Vec<GpuPass>,
    ) {
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|s| s.state.load(Ordering::Acquire) == FREE)
        else {
            return;
        };
        let bytes = passes.len() as u64 * 2 * std::mem::size_of::<u64>() as u64;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("GPU Timestamp Resolve Encoder"),
        });
        encoder.resolve_query_set(set, 0..passes.len() as u32 * 2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &slot.buffer, 0, Some(bytes));
        queue.submit(Some(encoder.finish()));

        self.submitted += 1;
        slot.frame = self.submitted;
        slot.passes = passes;
        slot.state.store(MAPPING, Ordering::Release);
        let state = Arc::clone(&slot.state);
        slot.buffer
            .map_async(wgpu::MapMode::Read, ..bytes, move |res| {
                state.store(if res.is_ok() { MAPPED } else { FREE }, Ordering::Release);
            });
    }

    /// Read every mapped slot through `read` (its ticks and passes), free them, and
    /// return the newest frame's result.
    pub fn read_finished<T>(&mut self, read: impl Fn(&[u64], &[GpuPass]) -> T) -> Option<T> {
        let mut newest: Option<(u64, T)> = None;
        for slot in &mut self.slots {
            if slot.state.load(Ordering::Acquire) != MAPPED {
                continue;
            }
            let bytes = slot.passes.len() as u64 * 2 * std::mem::size_of::<u64>() as u64;
            // Mapped by the callback, so the range is there; a wgpu error skips it.
            let result = slot
                .buffer
                .slice(..bytes)
                .get_mapped_range()
                .ok()
                .map(|view| {
                    let ticks: Vec<u64> = view
                        .chunks_exact(8)
                        .map(|b| u64::from_le_bytes(b.try_into().expect("8-byte chunk")))
                        .collect();
                    read(&ticks, &slot.passes)
                });
            slot.buffer.unmap();
            slot.state.store(FREE, Ordering::Release);
            let Some(result) = result else { continue };
            if newest.as_ref().is_none_or(|(f, _)| slot.frame > *f) {
                newest = Some((slot.frame, result));
            }
        }
        newest.map(|(_, r)| r)
    }
}
