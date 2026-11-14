//! How many instances a camera's GPU occlusion cull hid (#1963), and how many
//! clusters its cluster cull dropped (#1968): what the GPU drew copied to a mapped
//! buffer and read a frame or so later, never waited on, so `Debug.Stats`'
//! `hiz_culled` and `cluster_culled` trail the frame they describe.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// A read whose map has not answered yet.
const PENDING: u8 = 0;
/// A read whose copy is mapped and readable.
const MAPPED: u8 = 1;
/// A read whose map failed: nothing to read, and the next copy may go.
const FAILED: u8 = 2;

/// One camera's read back, at most one in flight.
#[derive(Default)]
pub(crate) struct Counter {
    staging: Option<wgpu::Buffer>,
    /// The items tested and bytes copied of the read in flight.
    in_flight: Option<(u32, u64)>,
    /// Set by the map callback: [`MAPPED`] once the copy is readable, [`FAILED`]
    /// when the map was refused; [`PENDING`] until then.
    mapped: Arc<AtomicU8>,
    /// Whether the copy was recorded this frame and waits for its submit.
    copied: bool,
    /// Items the last read found hidden.
    pub(crate) culled: u32,
}

impl Counter {
    /// How many instances final args (five words a batch) draw.
    pub(crate) fn drawn_instances(words: &[u32]) -> u32 {
        words.chunks_exact(5).map(|args| args[1]).sum()
    }

    /// The count in a buffer's first word.
    pub(crate) fn first_word(words: &[u32]) -> u32 {
        words.first().copied().unwrap_or(0)
    }

    /// Take a finished read's count, if one came back: `drawn` reads how many
    /// tested items were drawn from the copied words.
    pub(crate) fn collect(&mut self, device: &wgpu::Device, drawn: fn(&[u32]) -> u32) {
        let Some((tested, bytes)) = self.in_flight else {
            return;
        };
        // Lets a pending map's callback run, without waiting for it.
        let _ = device.poll(wgpu::PollType::Poll);
        if !self.settle() {
            return;
        }
        let staging = self
            .staging
            .as_ref()
            .expect("a read in flight has its buffer");
        let read = staging
            .slice(..bytes)
            .get_mapped_range()
            .map(|view| drawn(bytemuck::cast_slice(&view)));
        staging.unmap();
        if let Ok(drawn) = read {
            self.culled = tested.saturating_sub(drawn);
        }
        self.in_flight = None;
    }

    /// Whether the read in flight is mapped and readable now. A refused map
    /// leaves nothing to read: the read is dropped, so the next frame copies
    /// afresh rather than waiting on it for ever and `culled` keeps moving.
    fn settle(&mut self) -> bool {
        match self.mapped.swap(PENDING, Ordering::AcqRel) {
            MAPPED => true,
            FAILED => {
                self.in_flight = None;
                false
            }
            _ => false,
        }
    }

    /// The last read's count while the items it counts are still being tested
    /// (`active`), else `0`, forgotten: a frame that tests none culls none, and a
    /// read still in flight from before reports at most a frame or so late.
    pub(crate) fn while_active(&mut self, active: bool) -> u32 {
        if !active {
            self.culled = 0;
        }
        self.culled
    }

    /// Record a copy of `source`'s first `bytes` (a multiple of 4), when no read
    /// is in flight.
    pub(crate) fn copy(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        (source, bytes): (&wgpu::Buffer, u64),
    ) {
        self.copied = false;
        if self.in_flight.is_some() {
            return;
        }
        let bytes = bytes.max(4);
        if self.staging.as_ref().is_none_or(|s| s.size() < bytes) {
            self.staging = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Hi-Z Counter Read Back"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let staging = self.staging.as_ref().expect("made above");
        let copied = bytes.min(source.size());
        encoder.copy_buffer_to_buffer(source, 0, staging, 0, copied);
        self.copied = true;
        self.in_flight = Some((0, bytes));
    }

    /// After the copy's submit: map it, for `tested` items.
    pub(crate) fn request(&mut self, tested: u32) {
        if !std::mem::take(&mut self.copied) {
            return;
        }
        let Some((_, bytes)) = self.in_flight else {
            return;
        };
        self.in_flight = Some((tested, bytes));
        let staging = self.staging.as_ref().expect("copied into it");
        let mapped = Arc::clone(&self.mapped);
        staging
            .slice(..bytes)
            .map_async(wgpu::MapMode::Read, move |result| {
                let state = if result.is_ok() { MAPPED } else { FAILED };
                mapped.store(state, Ordering::Release);
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(state: u8) -> Counter {
        let counter = Counter {
            in_flight: Some((10, 20)),
            ..Counter::default()
        };
        counter.mapped.store(state, Ordering::Release);
        counter
    }

    #[test]
    fn a_count_is_zero_and_forgotten_once_nothing_is_tested() {
        let mut counter = Counter {
            culled: 7,
            ..Counter::default()
        };
        assert_eq!(counter.while_active(true), 7);
        assert_eq!(
            counter.while_active(false),
            0,
            "nothing tested, none culled"
        );
        assert_eq!(
            counter.while_active(true),
            0,
            "the stale 7 does not come back"
        );
    }

    #[test]
    fn a_failed_map_frees_the_read_and_a_pending_one_waits() {
        let mut failed = reading(FAILED);
        assert!(!failed.settle());
        assert_eq!(failed.in_flight, None, "the next copy may go");
        let mut pending = reading(PENDING);
        assert!(!pending.settle());
        assert_eq!(pending.in_flight, Some((10, 20)), "still waiting");
        let mut mapped = reading(MAPPED);
        assert!(mapped.settle());
        assert_eq!(mapped.mapped.load(Ordering::Acquire), PENDING);
    }
}
