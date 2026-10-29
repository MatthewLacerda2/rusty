//! src/audio/device/tap.rs — the final-mix tap a capture reads (#1579).
//!
//! The last effect on kira's main track, after the speaker-mode output stage: what
//! it sees is exactly what the device gets (every voice, group, filter, the reverb
//! bus, the compressor and limiter). While a capture runs it copies each block into
//! a shared buffer the engine drains once a frame; otherwise it costs one atomic
//! load per block. Platform layer only.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;

/// The tap's shared state: the audio thread writes, the engine drains.
#[derive(Debug, Default)]
pub struct Tap {
    on: AtomicBool,
    /// The rate the device runs at, in Hz (`0` until a block was tapped).
    rate: AtomicU32,
    /// Interleaved stereo samples tapped since the last [`Tap::take`].
    samples: Mutex<Vec<f32>>,
}

impl Tap {
    /// Start copying the mix, dropping anything left from an earlier capture.
    pub fn start(&self) {
        self.lock().clear();
        self.on.store(true, Ordering::Release);
    }

    /// Stop copying; what was tapped stays for a last [`Tap::take`].
    pub fn stop(&self) {
        self.on.store(false, Ordering::Release);
    }

    /// Everything tapped since the last call, interleaved stereo.
    pub fn take(&self) -> Vec<f32> {
        std::mem::take(&mut *self.lock())
    }

    /// The device's sample rate, once it has rendered a block.
    pub fn rate(&self) -> Option<u32> {
        Some(self.rate.load(Ordering::Acquire)).filter(|&r| r > 0)
    }

    /// The buffer; a panic elsewhere never poisons a capture for good.
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<f32>> {
        self.samples.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Copy one processed block, if a capture is running.
    fn copy(&self, block: &[Frame], dt: f64) {
        if !self.on.load(Ordering::Acquire) {
            return;
        }
        if dt > 0.0 {
            self.rate
                .store((1.0 / dt).round() as u32, Ordering::Release);
        }
        let mut samples = self.lock();
        for frame in block {
            samples.extend_from_slice(&[frame.left, frame.right]);
        }
    }
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
