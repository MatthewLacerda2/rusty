//! crates/render/src/gpu/grow_buffer.rs — a GPU buffer rewritten wholesale every frame, grown
//! (never shrunk) when a frame's contents outgrow it (#470).
//!
//! The instanced draw lists pack a whole frame's per-draw uniforms, bone palettes and
//! instances into a few of these, so a frame costs a handful of `write_buffer` calls
//! instead of one per entity. Growth reallocates, which invalidates any bind group that
//! names the old buffer — `upload` says so, and the owner rebuilds. The object table
//! (#1706) is kept instead: [`GrowBuffer::reserve`] grows it, and
//! [`GrowBuffer::write_at`] writes only the slots that changed; and
//! [`GrowBuffer::reserve_kept`] grows one whose contents the GPU wrote (#1963).

/// Smallest allocation, so a near-empty scene does not reallocate on its first props.
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

    /// Write `bytes` at offset 0, first reallocating to the next power of two when they
    /// do not fit. Returns `true` when the buffer was replaced, so the caller must
    /// rebuild whatever bind group referenced it.
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
            queue.write_buffer(&self.buffer, 0, bytes);
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

    /// Make room for `bytes`, keeping what the buffer holds: on growth its old
    /// contents are copied into the new one's start (a submit of their own), and
    /// the old size is returned, so the caller writes only past it and rebuilds its
    /// bind groups. Needs `COPY_SRC` among the usages.
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
        queue.submit(std::iter::once(encoder.finish()));
        self.buffer = grown;
        Some(old)
    }

    /// Write `bytes` at `offset`, which [`Self::reserve`] made room for.
    pub(crate) fn write_at(&self, queue: &wgpu::Queue, offset: u64, bytes: &[u8]) {
        debug_assert!(offset + bytes.len() as u64 <= self.buffer.size());
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, offset, bytes);
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
