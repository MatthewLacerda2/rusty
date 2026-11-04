//! src/render/gpu/draw_buffers/objects.rs — the object array on the GPU (#1706): the
//! renderer's `scene::ObjectTable` mirrored into one storage buffer, written where it
//! changed rather than packed per camera.

use super::{DrawBuffers, ObjectData};
use crate::render::scene::ObjectTable;

/// One record's bytes: the array's stride.
const STRIDE: u64 = std::mem::size_of::<ObjectData>() as u64;

impl DrawBuffers {
    /// Bring the GPU's object array up to `table`: its dirty slots, in runs, then the
    /// current camera's transient ones past the kept slots. When the array must grow,
    /// the new buffer gets every slot instead, and this returns `true` so the bind
    /// group is rebuilt over it. Adds the bytes it wrote to `written`.
    pub(super) fn sync_objects(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        table: &mut ObjectTable,
    ) -> bool {
        let runs = table.take_dirty();
        let (kept, transient) = table.records();
        let bytes = (kept.len() + transient.len()) as u64 * STRIDE;
        let grew = self.objects.reserve(device, bytes);
        if grew {
            self.objects.write_at(queue, 0, bytemuck::cast_slice(kept));
            self.written += kept.len() as u64 * STRIDE;
        } else {
            for run in runs {
                let slots = &kept[run.start as usize..run.end as usize];
                let at = u64::from(run.start) * STRIDE;
                self.objects
                    .write_at(queue, at, bytemuck::cast_slice(slots));
                self.written += slots.len() as u64 * STRIDE;
            }
        }
        let after = kept.len() as u64 * STRIDE;
        self.objects
            .write_at(queue, after, bytemuck::cast_slice(transient));
        self.written += transient.len() as u64 * STRIDE;
        grew
    }
}

#[cfg(test)]
impl crate::render::Renderer {
    /// Write the object table's dirty slots now, as a camera's upload would, and
    /// read the GPU's kept slots back.
    pub(crate) fn objects_on_gpu(&mut self) -> Vec<ObjectData> {
        let (device, queue) = (&self.device, &self.queue);
        let buffers = &mut self.draw_buffers;
        if buffers.sync_objects(device, queue, &mut self.retained.objects) {
            buffers.rebind(device, &self.entity_bones_layout);
        }
        let size = self.retained.objects.records().0.len() as u64 * STRIDE;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Object Records Readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffers.objects.buffer(), 0, &staging, 0, size);
        queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let mapped = slice.get_mapped_range().expect("mapped");
        bytemuck::cast_slice(&mapped).to_vec()
    }
}
