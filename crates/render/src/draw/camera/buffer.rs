//! crates/render/src/draw/camera/buffer.rs — the camera uniform group 0 binds, and
//! the copies that rewrite it inside a camera's one encoder (#2353).
//!
//! A camera uploads its uniform once, before anything records. Two stages change
//! it mid-camera: SSIL's source clears the `ssil` flag after the opaque pass, and
//! the game gizmos draw through the base camera and then put the camera's own back.
//! A `write_buffer` would land at the camera's submit, ahead of every pass it
//! recorded, so each of them writes its value into its own [`Slot`] of a staging
//! buffer and records a copy from there into the uniform, at its place among the
//! passes.

use crate::gpu::ops::CountedQueue as _;
use crate::CameraUniform;

/// A mid-camera value of the uniform, each written at most once a camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// The camera after SSIL's source: its `ssil` flag clear.
    SsilCleared,
    /// The base camera's, which the game gizmos draw through.
    GizmoBase,
    /// The camera's own again, after the gizmos.
    GizmoBack,
}

const SLOTS: u64 = 3;
const SIZE: u64 = std::mem::size_of::<CameraUniform>() as u64;

/// The camera uniform and its mid-camera slots.
pub(crate) struct CameraBuffer {
    /// What group 0 binds.
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

    /// Upload a camera's uniform, before its encoder records anything that reads it.
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
