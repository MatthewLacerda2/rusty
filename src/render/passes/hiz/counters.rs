//! How many instances a camera's GPU occlusion cull hid (#1963): its final args
//! copied to a mapped buffer and read a frame or so later, never waited on, so
//! `Debug.Stats`' `hiz_culled` trails the frame it describes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::cull::ARGS;

/// One camera's read back, at most one in flight.
#[derive(Default)]
pub(crate) struct Counter {
    staging: Option<wgpu::Buffer>,
    /// The instances tested and batches copied of the read in flight.
    in_flight: Option<(u32, u32)>,
    /// Set by the map callback once the copy is readable.
    mapped: Arc<AtomicBool>,
    /// Whether the copy was recorded this frame and waits for its submit.
    copied: bool,
    /// Instances the last read found hidden.
    pub(crate) culled: u32,
}

impl Counter {
    /// Take a finished read's count, if one came back.
    pub(super) fn collect(&mut self, device: &wgpu::Device) {
        let Some((tested, batches)) = self.in_flight else {
            return;
        };
        // Lets a pending map's callback run, without waiting for it.
        let _ = device.poll(wgpu::PollType::Poll);
        if !self.mapped.swap(false, Ordering::AcqRel) {
            return;
        }
        let staging = self
            .staging
            .as_ref()
            .expect("a read in flight has its buffer");
        let bytes = (ARGS * u64::from(batches)).max(4);
        let drawn = staging.slice(..bytes).get_mapped_range().map(|view| {
            let words: &[u32] = bytemuck::cast_slice(&view);
            words.chunks_exact(5).map(|args| args[1]).sum::<u32>()
        });
        staging.unmap();
        if let Ok(drawn) = drawn {
            self.culled = tested.saturating_sub(drawn);
        }
        self.in_flight = None;
    }

    /// Record a copy of `args`' first `batches` draws, when no read is in flight.
    pub(super) fn copy(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        (args, batches): (&wgpu::Buffer, u32),
    ) {
        self.copied = false;
        if self.in_flight.is_some() {
            return;
        }
        let bytes = (ARGS * u64::from(batches)).max(4);
        if self.staging.as_ref().is_none_or(|s| s.size() < bytes) {
            self.staging = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Hi-Z Counter Read Back"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let staging = self.staging.as_ref().expect("made above");
        encoder.copy_buffer_to_buffer(args, 0, staging, 0, ARGS * u64::from(batches));
        self.copied = true;
        self.in_flight = Some((0, batches));
    }

    /// After the copy's submit: map it, for `tested` instances.
    pub(super) fn request(&mut self, tested: u32) {
        if !std::mem::take(&mut self.copied) {
            return;
        }
        let Some((_, batches)) = self.in_flight else {
            return;
        };
        self.in_flight = Some((tested, batches));
        let staging = self.staging.as_ref().expect("copied into it");
        let mapped = Arc::clone(&self.mapped);
        let bytes = (ARGS * u64::from(batches)).max(4);
        staging
            .slice(..bytes)
            .map_async(wgpu::MapMode::Read, move |result| {
                mapped.store(result.is_ok(), Ordering::Release);
            });
    }
}
