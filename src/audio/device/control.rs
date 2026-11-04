//! src/audio/device/control.rs — a voice's live mix, shared with the audio thread.
//!
//! The backend writes a [`VoiceControl`] once a frame; the voice
//! (`device/voice.rs`) reads it on the audio thread at its next buffer. Plain
//! relaxed atomics: each field is one value, read whole, and a field read a buffer
//! late only delays that change by a few ms.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::audio::backend::VoiceMix;

/// An `f32` shared between the backend (writer) and the audio thread (reader).
#[derive(Debug, Default)]
pub(super) struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub(super) fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
    pub(super) fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// The live state of one voice, shared between the backend and the audio thread.
#[derive(Debug, Default)]
pub struct VoiceControl {
    pub(super) gain: AtomicF32,
    pub(super) pan: AtomicF32,
    pub(super) spatial_blend: AtomicF32,
    pub(super) speed: AtomicF32,
    /// The doppler rate (#1749), which the voice glides.
    pub(super) doppler: AtomicF32,
    pub(super) paused: AtomicBool,
    /// The voice's low-pass cutoff in Hz (occlusion, #467).
    pub(super) low_pass: AtomicF32,
    /// Set by the backend to end the voice; kira then unloads it.
    pub(super) stopped: AtomicBool,
    /// Set by the audio thread when a one-shot reaches its end.
    pub(super) finished: AtomicBool,
}

impl VoiceControl {
    /// A control starting at `mix`.
    pub fn new(mix: &VoiceMix) -> Arc<Self> {
        let control = Arc::new(Self::default());
        control.set(mix);
        control
    }

    /// Retune the voice; clamps every field into range.
    pub fn set(&self, mix: &VoiceMix) {
        self.gain.set(mix.gain.max(0.0));
        self.pan.set(mix.pan.clamp(-1.0, 1.0));
        self.spatial_blend.set(mix.spatial_blend.clamp(0.0, 1.0));
        self.speed.set(mix.speed.max(0.0));
        let doppler = mix.doppler;
        self.doppler.set(if doppler.is_finite() {
            doppler.max(0.0)
        } else {
            1.0
        });
        self.paused.store(mix.paused, Ordering::Relaxed);
        self.low_pass.set(mix.low_pass);
    }

    /// End the voice at the audio thread's next buffer.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    /// Whether the voice is still producing sound (not stopped, not run out).
    pub fn is_live(&self) -> bool {
        !self.stopped.load(Ordering::Relaxed) && !self.finished.load(Ordering::Relaxed)
    }
}
