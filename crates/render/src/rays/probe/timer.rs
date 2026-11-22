//! One timestamp pair around a compute pass, read as `timing::clock::ClockProbe` does.

/// The GPU's own timestamps, where the adapter has them.
pub(super) struct Timer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period_ns: f64,
}

impl Timer {
    /// `None` on an adapter without `TIMESTAMP_QUERY`.
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 16,
                usage,
                mapped_at_creation: false,
            })
        };
        use wgpu::BufferUsages as U;
        Some(Self {
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("Ray Probe Timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: buffer("Ray Probe Resolve", U::QUERY_RESOLVE | U::COPY_SRC),
            readback: buffer("Ray Probe Readback", U::MAP_READ | U::COPY_DST),
            period_ns: f64::from(queue.get_timestamp_period()),
        })
    }

    /// The pass's writes: its start and its end.
    pub(super) fn writes(&self) -> wgpu::ComputePassTimestampWrites<'_> {
        wgpu::ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }
    }

    /// Records the pair's resolve into `encoder`, after the pass.
    pub(super) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.set, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, Some(16));
    }

    /// The pass's milliseconds, once the encoder `resolve` recorded into was submitted.
    pub(super) fn read(&self, device: &wgpu::Device) -> Option<f64> {
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let ticks: Vec<u64> = (slice.get_mapped_range().ok()?.chunks_exact(8))
            .map(|b| u64::from_le_bytes(b.try_into().expect("8-byte chunk")))
            .collect();
        self.readback.unmap();
        let ms = ticks[1].saturating_sub(ticks[0]) as f64 * self.period_ns / 1e6;
        (ms > 0.0).then_some(ms)
    }
}
