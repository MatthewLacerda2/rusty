//! src/audio/device/master.rs — the master bus: the mix's single output stage (#546).
//!
//! Every voice's sink feeds one stereo mixer, and that summed signal reaches the
//! device through a [`MasterBus`] — the one place that sees the *total*, so a
//! footstep and an explosion playing together are processed as one signal. The
//! speaker mode's summed-output half lives here:
//!
//!   * **Home theater** — samples pass through untouched (bit-identical).
//!   * **Headphones** — a light crossfeed ([`CROSSFEED`]) bleeds each channel into
//!     the other, narrowing clips that are wide on their own; per-voice pan
//!     narrowing is `SpeakerMode::shape`.
//!   * **TV** — a stereo-linked [`Compressor`] (downward above the threshold, makeup
//!     gain lifting everything below it) then a soft [`limit`] so the lifted mix
//!     never clips.
//!
//! The bus is also the future home of mixer groups (SFX / music / UI). The mode is
//! read from a shared [`MasterControl`] on the audio thread, like `PanControl`.
//! Platform layer only; nothing in the sim reaches it.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;

use crate::audio::SpeakerMode;

/// How much of each channel headphones mode bleeds into the other.
pub const CROSSFEED: f32 = 0.15;

/// TV compressor: level (linear peak) above which gain is reduced (−24 dBFS).
const THRESHOLD: f32 = 0.063;
/// TV compressor: input dB above the threshold per output dB.
const RATIO: f32 = 4.0;
/// TV compressor: gain applied after compression (+9 dB), lifting quiet cues.
const MAKEUP: f32 = 2.818;
/// Envelope attack / release times, in seconds.
const ATTACK_S: f32 = 0.005;
const RELEASE_S: f32 = 0.2;
/// The soft limiter is linear up to here, then eases toward (never past) 1.0.
const LIMIT_KNEE: f32 = 0.9;

/// The live speaker mode, shared between the backend (writer) and the audio thread.
#[derive(Debug)]
pub struct MasterControl(AtomicU8);

impl MasterControl {
    /// A control starting at `mode`.
    pub fn new(mode: SpeakerMode) -> Arc<Self> {
        let control = Arc::new(Self(AtomicU8::new(0)));
        control.set(mode);
        control
    }

    pub fn set(&self, mode: SpeakerMode) {
        let index = SpeakerMode::ALL.iter().position(|m| *m == mode);
        self.0
            .store(index.unwrap_or_default() as u8, Ordering::Relaxed);
    }

    pub fn get(&self) -> SpeakerMode {
        let index = usize::from(self.0.load(Ordering::Relaxed));
        SpeakerMode::ALL.get(index).copied().unwrap_or_default()
    }
}

/// A stereo-linked peak compressor with makeup gain (TV mode).
#[derive(Clone, Debug)]
pub struct Compressor {
    envelope: f32,
    attack: f32,
    release: f32,
}

impl Compressor {
    /// A compressor running at `sample_rate` frames per second.
    pub fn new(sample_rate: u32) -> Self {
        let rate = sample_rate.max(1) as f32;
        Self {
            envelope: 0.0,
            attack: (-1.0 / (ATTACK_S * rate)).exp(),
            release: (-1.0 / (RELEASE_S * rate)).exp(),
        }
    }

    /// Compress one stereo frame, then soft-limit it.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs());
        let coeff = if peak > self.envelope {
            self.attack
        } else {
            self.release
        };
        self.envelope = coeff * self.envelope + (1.0 - coeff) * peak;
        let reduction = if self.envelope > THRESHOLD {
            (self.envelope / THRESHOLD).powf(1.0 / RATIO - 1.0)
        } else {
            1.0
        };
        let gain = reduction * MAKEUP;
        (limit(l * gain), limit(r * gain))
    }
}

/// Soft limiter: identity up to 0.9, then a `tanh` ease that never passes 1.
pub fn limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= LIMIT_KNEE {
        return x;
    }
    let room = 1.0 - LIMIT_KNEE;
    (LIMIT_KNEE + room * ((a - LIMIT_KNEE) / room).tanh()).copysign(x)
}

/// Shape one summed stereo frame for `mode`. See the module docs.
pub fn process_frame(mode: SpeakerMode, comp: &mut Compressor, l: f32, r: f32) -> (f32, f32) {
    match mode {
        SpeakerMode::HomeTheater => (l, r),
        SpeakerMode::Headphones => (
            l * (1.0 - CROSSFEED) + r * CROSSFEED,
            r * (1.0 - CROSSFEED) + l * CROSSFEED,
        ),
        SpeakerMode::Tv => comp.process(l, r),
    }
}

/// The stereo output stage wrapping the voices' mixer. Never ends: while no voice is
/// live it plays silence, so the device keeps the bus (voices join the mixer later).
pub struct MasterBus<S> {
    inner: S,
    control: Arc<MasterControl>,
    comp: Compressor,
    pending_right: Option<f32>,
}

impl<S: Source<Item = f32>> MasterBus<S> {
    /// `inner` must be stereo (the backend's mixer is built with two channels).
    pub fn new(inner: S, control: Arc<MasterControl>) -> Self {
        let comp = Compressor::new(inner.sample_rate());
        Self {
            inner,
            control,
            comp,
            pending_right: None,
        }
    }
}

impl<S: Source<Item = f32>> Iterator for MasterBus<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.pending_right.take() {
            return Some(right);
        }
        let l = self.inner.next().unwrap_or(0.0);
        let r = self.inner.next().unwrap_or(0.0);
        let (l, r) = process_frame(self.control.get(), &mut self.comp, l, r);
        self.pending_right = Some(r);
        Some(l)
    }
}

impl<S: Source<Item = f32>> Source for MasterBus<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
#[path = "master_tests.rs"]
mod master_tests;
