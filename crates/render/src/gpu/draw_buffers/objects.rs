use super::super::grow_buffer::GrowBuffer;
use super::{DrawBuffers, ObjectData};
use crate::scene::{SlotTable, Slotted};

/// One object record's bytes: the object array's stride.
#[cfg(test)]
const STRIDE: u64 = std::mem::size_of::<ObjectData>() as u64;

pub(super) fn sync_table<T: Slotted>(
    (device, queue): (&wgpu::Device, &wgpu::Queue),
    buffer: &mut GrowBuffer,
    table: &mut SlotTable<T>,
    written: &mut u64,
) -> bool {
    let stride = std::mem::size_of::<T>() as u64;
    let runs = table.take_dirty();
    let (kept, transient) = table.records();
    let bytes = (kept.len() + transient.len()) as u64 * stride;
    let grew = buffer.reserve(device, bytes);
    if grew {
        buffer.write_at(queue, 0, bytemuck::cast_slice(kept));
        *written += kept.len() as u64 * stride;
    } else {
        for run in runs {
            let slots = &kept[run.start as usize..run.end as usize];
            buffer.write_at(
                queue,
                u64::from(run.start) * stride,
                bytemuck::cast_slice(slots),
            );
            *written += slots.len() as u64 * stride;
        }
    }
    let after = kept.len() as u64 * stride;
    buffer.write_at(queue, after, bytemuck::cast_slice(transient));
    *written += transient.len() as u64 * stride;
    grew
}

impl DrawBuffers {
    pub(super) fn sync_objects(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        table: &mut SlotTable<ObjectData>,
    ) -> bool {
        sync_table((device, queue), &mut self.objects, table, &mut self.written)
    }
}

#[cfg(test)]
impl crate::Renderer {
    pub(crate) fn objects_on_gpu(&mut self) -> Vec<ObjectData> {
        let (device, queue) = (&self.device, &self.queue);
        let buffers = &mut self.draw_buffers;
        if buffers.sync_objects(device, queue, &mut self.retained.objects) {
            buffers.rebind(device, &self.entity_bones_layout);
        }
        let size = self.retained.objects.records().0.len() as u64 * STRIDE;
        read_back(device, queue, buffers.objects.buffer(), size)
    }

    pub(crate) fn records_on_gpu(&mut self) -> Vec<crate::MaterialData> {
        let (device, queue) = (&self.device, &self.queue);
        let buffers = &mut self.draw_buffers;
        let table = self.retained.records.table_for_sync();
        if sync_table((device, queue), &mut buffers.materials, table, &mut 0) {
            buffers.rebind(device, &self.entity_bones_layout);
        }
        let stride = std::mem::size_of::<crate::MaterialData>() as u64;
        let size = table.records().0.len() as u64 * stride;
        read_back(device, queue, buffers.materials.buffer(), size)
    }
}

/// The first `size` bytes of `buffer`, read back as records.
#[cfg(test)]
fn read_back<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    size: u64,
) -> Vec<T> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Slot Table Readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size);
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let mapped = slice.get_mapped_range().expect("mapped");
    bytemuck::cast_slice(&mapped).to_vec()
}
