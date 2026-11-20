//! Every clustered mesh's cluster table and indices in one place (#2417), so one
//! binding (`hiz_clusters.wgsl`'s group 1) reaches them all and a frame's
//! cluster cull is one dispatch a stage whatever its meshes.
//!
//! - **Two buffers, shared by the renderer**: the table (a 64-byte `Cluster` row
//!   each) and the index pool, a copy of each clustered mesh's reordered indices
//!   for the copy stage to read (the draws keep reading the mesh's own index
//!   buffer: a mesh's whole draws, its shadows and phase one bind it as before).
//! - **A mesh takes a range of each** when its clusters are attached
//!   ([`ClusterPool::lease`]): its rows carry their first index into the pool,
//!   so the shader adds no base. The [`Lease`] gives both back when it is
//!   dropped, with its `GpuClusters`: a re-upload, a mesh replaced by a bigger
//!   one, clusters dropped by an edit. The next lease reuses the hole (first
//!   fit, `ranges`); nothing is ever moved, so a range's offsets hold for life.
//!   A scene load frees nothing by itself: meshes stay resident across scenes.
//! - **Growth** doubles a buffer and copies what it held on the GPU (one copy in
//!   a submit of its own, no CPU wait), then rebuilds the one bind group: a
//!   Dust2-sized level's first upload grows each buffer about log2(size / 4 KiB)
//!   times, every later frame none.
//! - **The limit is the session's, not a mesh's**: each buffer is bound whole,
//!   so it stays within the device's `max_storage_buffer_binding_size` (128 MiB
//!   by default: 33.5M pooled indices, 2M clusters), and that is the total over
//!   every clustered mesh resident at once. Meshes stay resident across scene
//!   loads, so a long session's meshes add up. A mesh that would pass it gets
//!   no lease, logs `does not fit the cluster pool`, and is drawn whole by the
//!   instance test; it is not retried when room frees later (releasing resident
//!   meshes and retrying are #2440's). Offsets are worked in `u64` and the limit
//!   clamped to what a `u32` index reaches, so a raised device limit cannot wrap
//!   a row onto another mesh's.

use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use rusty_asset::clusters::Clusters;

use super::ranges::Ranges;
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::ops::CountedDevice as _;

/// A `Cluster` row's bytes.
const ROW_BYTES: u64 = 64;

/// What each buffer has handed out, shared with the leases that free it.
#[derive(Default)]
struct Ledger {
    table: Ranges,
    indices: Ranges,
}

/// The renderer's one cluster table and index pool, and the group binding both.
pub(crate) struct ClusterPool {
    table: GrowBuffer,
    indices: GrowBuffer,
    ledger: Arc<Mutex<Ledger>>,
    /// The bytes either buffer may reach: the device's binding limit, lowered
    /// by a test to make a refusal cheap.
    pub(crate) limit: u64,
    pub(crate) group: wgpu::BindGroup,
}

/// A mesh's rows of the table and indices of the pool, given back on drop.
pub(crate) struct Lease {
    pub(crate) table: Range<u32>,
    pub(crate) indices: Range<u32>,
    ledger: Arc<Mutex<Ledger>>,
}

impl Ledger {
    /// `rows` of the table and `words` of the pool, each within its capacity:
    /// both or neither, so a table that refuses gives the pool's range back.
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

    /// Rows and indices for `clusters` over the mesh's `indices`, written into
    /// the pool: `None` when either would pass `limit`, with nothing taken.
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

    /// The rows and indices held by live leases, and the buffers' bytes.
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

/// The rows and pooled indices `limit` bytes hold, each clamped to what a `u32`
/// offset addresses.
fn capacity(limit: u64) -> (u32, u32) {
    let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    (clamp(limit / ROW_BYTES), clamp(limit / 4))
}

/// Group 1 over the table and the pool, each bound up to the binding limit
/// (a grown buffer is a power of two, which may pass a limit that is not).
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
