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
//! - **The limit**: each buffer is bound whole, so it stays within the device's
//!   `max_storage_buffer_binding_size` (128 MiB by default: 33.5M pooled
//!   indices, 2M clusters). A mesh that would pass it gets no lease, logs it,
//!   and is drawn whole by the instance test, as a mesh too big to bind was.

use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use rusty_asset::clusters::Clusters;

use super::ranges::Ranges;
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::ops::CountedDevice as _;

/// A `Cluster` row's bytes.
const ROW_BYTES: u32 = 64;

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
    pub(crate) group: wgpu::BindGroup,
}

/// A mesh's rows of the table and indices of the pool, given back on drop.
pub(crate) struct Lease {
    pub(crate) table: Range<u32>,
    pub(crate) indices: Range<u32>,
    ledger: Arc<Mutex<Ledger>>,
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
            group,
        }
    }

    /// Rows and indices for `clusters` over the mesh's `indices`, written into
    /// the pool: `None` when either would pass the device's binding limit.
    pub(crate) fn lease(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        layout: &wgpu::BindGroupLayout,
        (clusters, indices): (&Clusters, &[u32]),
    ) -> Option<Lease> {
        let limit = device.limits().max_storage_buffer_binding_size;
        let (rows, words) = ((limit / u64::from(ROW_BYTES)) as u32, (limit / 4) as u32);
        let mut ledger = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        let at = ledger.indices.take(indices.len() as u32, words)?;
        let Some(table) = ledger.table.take(clusters.list.len() as u32, rows) else {
            ledger.indices.free(at);
            return None;
        };
        let ends = (ledger.table.end, ledger.indices.end);
        drop(ledger);
        let lease = Lease {
            table,
            indices: at,
            ledger: Arc::clone(&self.ledger),
        };
        let grown = [
            self.table
                .reserve_kept(device, queue, u64::from(ends.0 * ROW_BYTES)),
            self.indices
                .reserve_kept(device, queue, 4 * u64::from(ends.1)),
        ];
        if grown.iter().any(Option::is_some) {
            let buffers = [self.table.buffer(), self.indices.buffer()];
            self.group = bind(device, layout, buffers);
        }
        let rows = super::gpu::pack(clusters, lease.indices.start);
        let row_at = u64::from(lease.table.start * ROW_BYTES);
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
