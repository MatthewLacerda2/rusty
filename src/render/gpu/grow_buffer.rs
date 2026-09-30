//! src/render/gpu/grow_buffer.rs — a GPU buffer rewritten wholesale every frame, grown
//! (never shrunk) when a frame's contents outgrow it (#470).
//!
//! The instanced draw lists pack a whole frame's per-draw uniforms, bone palettes and
//! instances into a few of these, so a frame costs a handful of `write_buffer` calls
//! instead of one per entity. Growth reallocates, which invalidates any bind group that
//! names the old buffer — `upload` says so, and the owner rebuilds.

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
