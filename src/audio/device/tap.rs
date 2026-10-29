//! src/audio/device/tap.rs — the final-mix tap a capture reads (#1579).
//!
//! The last effect on kira's main track, after the speaker-mode output stage: what
//! it sees is exactly what the device gets (every voice, group, filter, the reverb
//! bus, the compressor and limiter). While a capture runs it copies each block into
//! a ring the engine drains once a frame; otherwise it costs one atomic load per
//! block. Platform layer only.
//!
//! **The audio thread never waits and never allocates**: a capture is how a dropout
//! gets diagnosed, so it must not cause one. The ring is a single-producer,
//! single-consumer buffer of atomics (the audio thread writes, the engine drains),
//! allocated once on the main thread when the first capture starts. A block that
//! finds the ring full is dropped whole and counted, and the capture reports the
//! count when it stops. **The file keeps true time**: a dropped block leaves a gap
//! the next drain fills with as many silent frames, in the place it was lost. While
//! a gap is pending the producer writes nothing, so everything in the ring precedes
//! it and the drain knows where it goes.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;

/// Interleaved samples the ring holds: about five seconds of 48 kHz stereo, far
/// more than a frame drains, so only a stalled engine fills it.
pub const RING_SAMPLES: usize = 1 << 19;

/// The tap's shared state: the audio thread writes, the engine drains.
#[derive(Debug, Default)]
pub struct Tap {
    on: AtomicBool,
    /// The rate the device runs at, in Hz (`0` until a block was tapped).
    rate: AtomicU32,
    /// The ring of `f32` bits, made by the first [`Tap::start`].
    ring: OnceLock<Box<[AtomicU32]>>,
    /// Samples ever written (the producer's index) and ever read (the consumer's);
    /// both only grow, and a slot is `index % RING_SAMPLES`.
    written: AtomicUsize,
    read: AtomicUsize,
    /// Stereo frames dropped since [`Tap::start`] because the ring was full.
    dropped: AtomicU64,
    /// Dropped frames the drain has not yet written as silence; non-zero holds the
    /// producer off the ring.
    gap: AtomicU64,
}

impl Tap {
    /// Start copying the mix, dropping anything left from an earlier capture.
    /// Called on the main thread, so the ring is allocated here, never on the
    /// audio thread.
    pub fn start(&self) {
        self.ring
            .get_or_init(|| (0..RING_SAMPLES).map(|_| AtomicU32::new(0)).collect());
        // Discarding what is unread is a consumer-side move, safe while the
        // producer runs.
        let written = self.written.load(Ordering::Acquire);
        self.read.store(written, Ordering::Release);
        self.dropped.store(0, Ordering::Relaxed);
        self.gap.store(0, Ordering::Release);
        self.on.store(true, Ordering::Release);
    }

    /// Stop copying; what was tapped stays for a last [`Tap::drain_into`].
    pub fn stop(&self) {
        self.on.store(false, Ordering::Release);
    }

    /// Append everything tapped since the last drain to `out`, interleaved stereo,
    /// then the silence standing in for any block dropped after it.
    pub fn drain_into(&self, out: &mut Vec<f32>) {
        let Some(ring) = self.ring.get() else {
            return;
        };
        // The gap first: what it counts was dropped after every sample in the ring.
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

    /// The device's sample rate, once it has rendered a block.
    pub fn rate(&self) -> Option<u32> {
        Some(self.rate.load(Ordering::Acquire)).filter(|&r| r > 0)
    }

    /// Copy one processed block, if a capture is running: lock-free, and a block
    /// that does not fit, or comes while a gap is pending, is dropped whole and
    /// counted into the gap.
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

/// The effect kira runs: passes the block through untouched and copies it.
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
