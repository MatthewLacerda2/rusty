use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use crate::gpu::commands::Commands;

const PENDING: u8 = 0;
const MAPPED: u8 = 1;
const FAILED: u8 = 2;

#[derive(Default)]
pub(crate) struct Counter {
    staging: Option<wgpu::Buffer>,
    in_flight: Option<(u32, u64)>,
    mapped: Arc<AtomicU8>,
    copied: bool,
    pub(crate) culled: u32,
    pub(crate) drawn: u32,
    pub(crate) late: u32,
    pub(crate) triangles: Option<Triangles>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Read {
    pub drawn: u32,
    pub late: u32,
    pub triangles: Triangles,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Triangles {
    pub phase_one: u64,
    pub late: u64,
    pub last: u64,
}

impl Counter {
    pub(crate) fn first_word(words: &[u32]) -> Read {
        Read {
            drawn: words.first().copied().unwrap_or(0),
            ..Read::default()
        }
    }

    pub(crate) fn idle(&self) -> bool {
        self.in_flight.is_none()
    }

    pub(crate) fn collect(&mut self, device: &wgpu::Device, read: impl FnOnce(&[u32]) -> Read) {
        let Some((tested, bytes)) = self.in_flight else {
            return;
        };
        // Lets a pending map's callback run without waiting: `Wait` would stall the frame.
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
            self.triangles = Some(read.triangles);
        }
        self.in_flight = None;
    }

    /// The staging buffer goes too: a map still pending would refuse the next copy into it.
    pub(crate) fn forget(&mut self) {
        if self.in_flight.take().is_some() {
            self.staging = None;
            self.mapped = Arc::default();
        }
        self.copied = false;
        (self.culled, self.drawn, self.late) = (0, 0, 0);
        self.triangles = None;
    }

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

    pub(crate) fn while_active(&mut self, active: bool) -> u32 {
        if !active {
            self.culled = 0;
        }
        self.culled
    }

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
