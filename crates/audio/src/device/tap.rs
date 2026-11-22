use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;

/// Interleaved samples the ring holds: about five seconds of 48 kHz stereo, far
/// more than a frame drains, so only a stalled engine fills it.
pub const RING_SAMPLES: usize = 1 << 19;

#[derive(Debug, Default)]
pub struct Tap {
    on: AtomicBool,
    rate: AtomicU32,
    ring: OnceLock<Box<[AtomicU32]>>,
    written: AtomicUsize,
    read: AtomicUsize,
    dropped: AtomicU64,
    gap: AtomicU64,
}

impl Tap {
    /// Start copying the mix, dropping anything left from an earlier capture.
    /// Called on the main thread, so the ring is allocated here, never on the
    /// audio thread.
    pub fn start(&self) {
        self.ring
            .get_or_init(|| (0..RING_SAMPLES).map(|_| AtomicU32::new(0)).collect());
        let written = self.written.load(Ordering::Acquire);
        self.read.store(written, Ordering::Release);
        self.dropped.store(0, Ordering::Relaxed);
        self.gap.store(0, Ordering::Release);
        self.on.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.on.store(false, Ordering::Release);
    }

    pub fn drain_into(&self, out: &mut Vec<f32>) {
        let Some(ring) = self.ring.get() else {
            return;
        };
        let gap = self.gap.load(Ordering::Acquire);
        let written = self.written.load(Ordering::Acquire);
        let read = self.read.load(Ordering::Relaxed);
        out.extend((read..written).map(|i| f32::from_bits(slot(ring, i).load(Ordering::Relaxed))));
        self.read.store(written, Ordering::Release);
        if gap > 0 {
            out.resize(out.len() + gap as usize * 2, 0.0);
            self.gap.fetch_sub(gap, Ordering::AcqRel);
        }
    }

    /// Stereo frames dropped since the capture started, because the engine did not
    /// drain the ring in time.
    pub fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn rate(&self) -> Option<u32> {
        Some(self.rate.load(Ordering::Acquire)).filter(|&r| r > 0)
    }

    fn copy(&self, block: &[Frame], dt: f64) {
        if !self.on.load(Ordering::Acquire) {
            return;
        }
        let Some(ring) = self.ring.get() else {
            return;
        };
        if dt > 0.0 {
            self.rate
                .store((1.0 / dt).round() as u32, Ordering::Release);
        }
        let written = self.written.load(Ordering::Relaxed);
        let read = self.read.load(Ordering::Acquire);
        let pending = self.gap.load(Ordering::Acquire) > 0;
        if pending || RING_SAMPLES - (written - read) < block.len() * 2 {
            let frames = block.len() as u64;
            self.dropped.fetch_add(frames, Ordering::Relaxed);
            self.gap.fetch_add(frames, Ordering::AcqRel);
            return;
        }
        for (i, frame) in block.iter().enumerate() {
            let at = written + i * 2;
            slot(ring, at).store(frame.left.to_bits(), Ordering::Relaxed);
            slot(ring, at + 1).store(frame.right.to_bits(), Ordering::Relaxed);
        }
        self.written
            .store(written + block.len() * 2, Ordering::Release);
    }
}

/// The ring slot sample `index` lives in.
fn slot(ring: &[AtomicU32], index: usize) -> &AtomicU32 {
    &ring[index % RING_SAMPLES]
}

pub struct TapEffect(pub Arc<Tap>);

impl Effect for TapEffect {
    fn process(&mut self, input: &mut [Frame], dt: f64, _info: &Info) {
        self.0.copy(input, dt);
    }
}

impl EffectBuilder for TapEffect {
    type Handle = ();
    fn build(self) -> (Box<dyn Effect>, ()) {
        (Box::new(self), ())
    }
}

#[cfg(test)]
#[path = "tap_tests.rs"]
mod tap_tests;
