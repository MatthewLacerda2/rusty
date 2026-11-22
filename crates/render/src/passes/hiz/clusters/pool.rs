use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use rusty_asset::clusters::Clusters;

use super::ranges::Ranges;
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::ops::CountedDevice as _;

const ROW_BYTES: u64 = 64;

#[derive(Default)]
struct Ledger {
    table: Ranges,
    indices: Ranges,
}

pub(crate) struct ClusterPool {
    table: GrowBuffer,
    indices: GrowBuffer,
    ledger: Arc<Mutex<Ledger>>,
    pub(crate) limit: u64,
    pub(crate) group: wgpu::BindGroup,
}

pub(crate) struct Lease {
    pub(crate) table: Range<u32>,
    pub(crate) indices: Range<u32>,
    ledger: Arc<Mutex<Ledger>>,
}

impl Ledger {
    fn take(
        &mut self,
        (rows, words): (u32, u32),
        (row_cap, word_cap): (u32, u32),
    ) -> Option<(Range<u32>, Range<u32>)> {
        let at = self.indices.take(words, word_cap)?;
        let Some(table) = self.table.take(rows, row_cap) else {
            self.indices.free(at);
            return None;
        };
        Some((table, at))
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut ledger = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        ledger.table.free(self.table.clone());
        ledger.indices.free(self.indices.clone());
    }
}

impl ClusterPool {
    pub(crate) fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        use wgpu::BufferUsages as U;
        let usage = U::STORAGE | U::COPY_SRC;
        let table = GrowBuffer::new(device, "Cluster Table", usage);
        let indices = GrowBuffer::new(device, "Cluster Index Pool", usage);
        let group = bind(device, layout, [table.buffer(), indices.buffer()]);
        Self {
            table,
            indices,
            ledger: Arc::default(),
            limit: device.limits().max_storage_buffer_binding_size,
            group,
        }
    }

    pub(crate) fn lease(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        layout: &wgpu::BindGroupLayout,
        (clusters, indices): (&Clusters, &[u32]),
    ) -> Option<Lease> {
        let len = (
            u32::try_from(clusters.list.len()),
            u32::try_from(indices.len()),
        );
        let len = (len.0.ok()?, len.1.ok()?);
        let mut ledger = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        let (table, at) = ledger.take(len, capacity(self.limit))?;
        let ends = (ledger.table.end, ledger.indices.end);
        drop(ledger);
        let lease = Lease {
            table,
            indices: at,
            ledger: Arc::clone(&self.ledger),
        };
        let grown = [
            self.table
                .reserve_kept(device, queue, ROW_BYTES * u64::from(ends.0)),
            self.indices
                .reserve_kept(device, queue, 4 * u64::from(ends.1)),
        ];
        if grown.iter().any(Option::is_some) {
            let buffers = [self.table.buffer(), self.indices.buffer()];
            self.group = bind(device, layout, buffers);
        }
        let rows = super::gpu::pack(clusters, lease.indices.start);
        let row_at = ROW_BYTES * u64::from(lease.table.start);
        self.table
            .write_at(queue, row_at, bytemuck::cast_slice(&rows));
        let index_at = 4 * u64::from(lease.indices.start);
        self.indices
            .write_at(queue, index_at, bytemuck::cast_slice(indices));
        Some(lease)
    }

    pub(crate) fn leased_bytes(&self) -> u64 {
        let ledger = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        ROW_BYTES * u64::from(ledger.table.used()) + 4 * u64::from(ledger.indices.used())
    }

    #[cfg(test)]
    pub(crate) fn held(&self) -> ((u32, u32), (u64, u64)) {
        let ledger = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        let used = (ledger.table.used(), ledger.indices.used());
        (
            used,
            (self.table.buffer().size(), self.indices.buffer().size()),
        )
    }
}

fn capacity(limit: u64) -> (u32, u32) {
    let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    (clamp(limit / ROW_BYTES), clamp(limit / 4))
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: [&wgpu::Buffer; 2],
) -> wgpu::BindGroup {
    let limit = device.limits().max_storage_buffer_binding_size;
    let entries: Vec<_> = (0..)
        .zip(buffers)
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: wgpu::BufferSize::new(buffer.size().min(limit)),
            }),
        })
        .collect();
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Cluster Pool"),
        layout,
        entries: &entries,
    })
}

#[cfg(test)]
mod tests {
    use super::{capacity, Ledger};

    #[test]
    fn a_table_that_refuses_gives_the_pool_range_back() {
        let mut ledger = Ledger::default();
        assert_eq!(ledger.take((4, 100), (5, 120)), Some((0..4, 0..100)));
        assert_eq!(ledger.take((2, 10), (5, 120)), None, "the table is full");
        assert_eq!(ledger.indices.used(), 100, "the pool's range given back");
        assert_eq!(ledger.take((1, 30), (5, 120)), None, "the pool is full");
        assert_eq!(ledger.take((1, 20), (5, 120)), Some((4..5, 100..120)));
    }

    #[test]
    fn a_raised_limit_is_clamped_to_what_a_u32_offset_reaches() {
        assert_eq!(capacity(128 << 20), (2 << 20, 32 << 20), "wgpu's default");
        let words = capacity(16 << 30);
        assert_eq!(words, (1 << 28, u32::MAX), "16 GiB: 4G words do not fit");
        assert_eq!(capacity(u64::MAX), (u32::MAX, u32::MAX));
    }
}
