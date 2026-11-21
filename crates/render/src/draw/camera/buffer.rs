use crate::gpu::ops::CountedQueue as _;
use crate::CameraUniform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    SsilCleared,
    GizmoBase,
    GizmoBack,
}

const SLOTS: u64 = 3;
const SIZE: u64 = std::mem::size_of::<CameraUniform>() as u64;

pub(crate) struct CameraBuffer {
    pub uniform: wgpu::Buffer,
    slots: wgpu::Buffer,
}

impl CameraBuffer {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            uniform: buffer("Camera Uniform Buffer", SIZE, wgpu::BufferUsages::UNIFORM),
            slots: buffer(
                "Camera Uniform Slots",
                SIZE * SLOTS,
                wgpu::BufferUsages::COPY_SRC,
            ),
        }
    }

    #[track_caller]
    pub(crate) fn upload(&self, queue: &wgpu::Queue, uniform: &CameraUniform) {
        queue.counted_write_buffer(&self.uniform, 0, bytemuck::bytes_of(uniform));
    }

    /// Make the uniform `uniform` from here on in `encoder`'s order: written into
    /// `slot`, copied from there into the uniform.
    #[track_caller]
    pub(crate) fn record(
        &self,
        (queue, encoder): (&wgpu::Queue, &mut wgpu::CommandEncoder),
        slot: Slot,
        uniform: &CameraUniform,
    ) {
        let at = slot as u64 * SIZE;
        queue.counted_write_buffer(&self.slots, at, bytemuck::bytes_of(uniform));
        encoder.copy_buffer_to_buffer(&self.slots, at, &self.uniform, 0, Some(SIZE));
    }
}
