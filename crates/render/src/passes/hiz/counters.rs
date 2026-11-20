//! How many instances a camera's GPU occlusion cull hid (#1963), and how many
//! clusters its cluster cull dropped (#1968): what the GPU drew copied to a mapped
//! buffer and read a frame or so later, never waited on, so `Debug.Stats`'
//! `hiz_culled` and `cluster_culled` trail the frame they describe.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use crate::gpu::commands::Commands;

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
    /// Items the last read found drawn.
    pub(crate) drawn: u32,
    /// The second count the last read's words carried: the late set's drawn
    /// instances for an instance cull's (#2336), `0` for a cluster cull's.
    pub(crate) late: u32,
}

/// Reads what a copy's words drew: the items drawn, and a second count beside
/// them ([`Counter::late`]).
pub(crate) type Reader = fn(&[u32]) -> (u32, u32);

impl Counter {
    /// How many instances the final args and, after them, the late set's draw
    /// (five words a batch, as many batches each): one copy of both (#2336), so
    /// the two counts describe the same frame.
    pub(crate) fn final_and_late(words: &[u32]) -> (u32, u32) {
        let instances = |args: &[u32]| args.chunks_exact(5).map(|a| a[1]).sum();
        let (last, late) = words.split_at(words.len() / 10 * 5);
        (instances(last), instances(late))
    }

    /// The count in a buffer's first word.
    pub(crate) fn first_word(words: &[u32]) -> (u32, u32) {
        (words.first().copied().unwrap_or(0), 0)
    }

    /// Take a finished read's counts, if one came back: `read` reads how many
    /// tested items were drawn, and its second count, from the copied words.
    pub(crate) fn collect(&mut self, device: &wgpu::Device, read: Reader) {
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
            .map(|view| read(bytemuck::cast_slice(&view)));
        staging.unmap();
        if let Ok((drawn, late)) = read {
            self.culled = tested.saturating_sub(drawn);
            self.drawn = drawn;
            self.late = late;
        }
        self.in_flight = None;
    }

    /// Drop the read in flight and its counts, for a frame that tests nothing:
    /// a later [`Self::collect`] reads none of it. The staging buffer goes with
    /// it, since a map still pending would refuse the next copy into it, and so
    /// does the flag its callback sets.
    pub(crate) fn forget(&mut self) {
        if self.in_flight.take().is_some() {
            self.staging = None;
            self.mapped = Arc::default();
        }
        self.copied = false;
        (self.culled, self.drawn, self.late) = (0, 0, 0);
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

    /// Record a copy of `bytes` (a multiple of 4) of `source` from byte `at`, when
    /// no read is in flight.
    pub(crate) fn copy(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        (source, at, bytes): (&wgpu::Buffer, u64, u64),
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
        let copied = bytes.min(source.size().saturating_sub(at));
        encoder.copy_buffer_to_buffer(source, at, staging, 0, copied);
        self.copied = true;
        self.in_flight = Some((0, bytes));
    }

    /// Map the copy once `commands`, which recorded it, are submitted, for
    /// `tested` items.
    pub(crate) fn request(&mut self, tested: u32, commands: &mut Commands) {
        if !std::mem::take(&mut self.copied) {
            return;
        }
        let Some((_, bytes)) = self.in_flight else {
            return;
        };
        self.in_flight = Some((tested, bytes));
        let staging = self.staging.clone().expect("copied into it");
        let mapped = Arc::clone(&self.mapped);
        commands.after_send(move || {
            staging
                .slice(..bytes)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let state = if result.is_ok() { MAPPED } else { FAILED };
                    mapped.store(state, Ordering::Release);
                });
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
    fn a_forgotten_read_is_never_collected_and_frees_the_next_copy() {
        let mut counter = reading(MAPPED);
        counter.culled = 7;
        counter.forget();
        assert_eq!(counter.in_flight, None, "the next copy may go");
        assert_eq!(counter.culled, 0, "no count from before the skip");
        assert!(counter.staging.is_none(), "a pending map keeps no buffer");
        assert_eq!(counter.mapped.load(Ordering::Acquire), PENDING);
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

    #[test]
    fn one_copy_of_both_args_counts_the_final_and_the_late_draws_apart() {
        // Two batches: final args drawing 3 and 4, then late args drawing 1 and 0.
        let words = [6, 3, 0, 0, 0, 6, 4, 0, 0, 3, 6, 1, 0, 0, 3, 6, 0, 0, 0, 7];
        assert_eq!(Counter::final_and_late(&words), (7, 1));
        assert_eq!(Counter::final_and_late(&words[..10]), (3, 4), "one batch");
        assert_eq!(Counter::final_and_late(&[]), (0, 0));
    }
}
