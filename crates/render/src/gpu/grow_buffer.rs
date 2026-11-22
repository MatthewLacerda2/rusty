use crate::gpu::ops::CountedQueue as _;

/// Bytes: so a near-empty scene does not reallocate on its first props.
const MIN_BYTES: u64 = 4096;

pub(crate) struct GrowBuffer {
    buffer: wgpu::Buffer,
    label: &'static str,
    usage: wgpu::BufferUsages,
}

impl GrowBuffer {
    pub(crate) fn new(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        Self {
            buffer: allocate(device, label, usage, MIN_BYTES),
            label,
            usage,
        }
    }

    pub(crate) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Write `bytes` at offset 0. Returns `true` when the buffer was replaced, so the
    /// caller must rebuild whatever bind group referenced it.
    #[track_caller]
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> bool {
        let needed = bytes.len() as u64;
        let grown = needed > self.buffer.size();
        if grown {
            let size = needed.next_power_of_two().max(MIN_BYTES);
            self.buffer = allocate(device, self.label, self.usage, size);
        }
        if !bytes.is_empty() {
            queue.counted_write_buffer(&self.buffer, 0, bytes);
        }
        grown
    }

    /// Make room for `bytes` without writing them: `true` when the buffer was
    /// replaced, empty, so the caller must rewrite all of it and rebuild its bind
    /// groups.
    pub(crate) fn reserve(&mut self, device: &wgpu::Device, bytes: u64) -> bool {
        let grown = bytes > self.buffer.size();
        if grown {
            let size = bytes.next_power_of_two().max(MIN_BYTES);
            self.buffer = allocate(device, self.label, self.usage, size);
        }
        grown
    }

    /// Grow, keeping the contents (copied in a submit of its own): `Some(old size)`, so
    /// the caller writes past it and rebuilds bind groups.
    pub(crate) fn reserve_kept(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: u64,
    ) -> Option<u64> {
        let old = self.buffer.size();
        if bytes <= old {
            return None;
        }
        debug_assert!(self.usage.contains(wgpu::BufferUsages::COPY_SRC));
        let size = bytes.next_power_of_two().max(MIN_BYTES);
        let grown = allocate(device, self.label, self.usage, size);
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.buffer, 0, &grown, 0, old);
        queue.counted_submit(std::iter::once(encoder.finish()));
        self.buffer = grown;
        Some(old)
    }

    /// Write `bytes` at `offset`, which [`Self::reserve`] made room for.
    #[track_caller]
    pub(crate) fn write_at(&self, queue: &wgpu::Queue, offset: u64, bytes: &[u8]) {
        debug_assert!(offset + bytes.len() as u64 <= self.buffer.size());
        if !bytes.is_empty() {
            queue.counted_write_buffer(&self.buffer, offset, bytes);
        }
    }
}

fn allocate(
    device: &wgpu::Device,
    label: &'static str,
    usage: wgpu::BufferUsages,
    size: u64,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}
