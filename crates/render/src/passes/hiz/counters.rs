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
    /// The triangles the last read's draws drew (#2387), by pass; none for a
    /// cluster cull's.
    pub(crate) triangles: Triangles,
}

/// What a read's words say: the items drawn, a second count beside them
/// ([`Counter::late`]) and the triangles drawn ([`Counter::triangles`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Read {
    pub drawn: u32,
    pub late: u32,
    pub triangles: Triangles,
}

/// The triangles a camera's indirect draws drew (#2387), read from their args:
/// phase one's depth, the late set's (drawn by the depth prepass, where it
/// runs) and the final set's (the forward pass's, by cluster where it draws so).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Triangles {
    pub phase_one: u64,
    pub late: u64,
    pub last: u64,
}

impl Counter {
    /// The count in a buffer's first word.
    pub(crate) fn first_word(words: &[u32]) -> Read {
        Read {
            drawn: words.first().copied().unwrap_or(0),
            ..Read::default()
        }
    }

    /// Whether no read is in flight, so the next [`Self::copy`] copies.
    pub(crate) fn idle(&self) -> bool {
        self.in_flight.is_none()
    }

    /// Take a finished read's counts, if one came back: `read` reads how many
    /// tested items were drawn, its second count and its triangles, from the
    /// copied words.
    pub(crate) fn collect(&mut self, device: &wgpu::Device, read: impl FnOnce(&[u32]) -> Read) {
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
        if let Ok(read) = read {
            self.culled = tested.saturating_sub(read.drawn);
            (self.drawn, self.late) = (read.drawn, read.late);
            self.triangles = read.triangles;
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
        self.triangles = Triangles::default();
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

    /// Record a copy of `segments`, each `(source, from byte, bytes)` (multiples
    /// of 4), one after another into one staging buffer, when no read is in
    /// flight: one read back of them all, so their counts describe one frame.
    pub(crate) fn copy(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        segments: &[(&wgpu::Buffer, u64, u64)],
    ) {
        self.copied = false;
        if self.in_flight.is_some() {
            return;
        }
        let bytes = segments.iter().map(|s| s.2).sum::<u64>().max(4);
        if self.staging.as_ref().is_none_or(|s| s.size() < bytes) {
            self.staging = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Hi-Z Counter Read Back"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let staging = self.staging.as_ref().expect("made above");
        let mut to = 0;
        for &(source, at, wanted) in segments {
            let copied = wanted.min(source.size().saturating_sub(at));
            if copied > 0 {
                encoder.copy_buffer_to_buffer(source, at, staging, to, copied);
            }
            to += wanted;
        }
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
}
