//! src/audio/device/pan.rs — the stereo panner the real device plays through (#412).
//!
//! `rodio::Sink` has volume and speed but no pan, so every voice's clip is wrapped in
//! a [`PanSource`] that always emits **stereo** and reads its pan and `spatial_blend`
//! from a shared [`PanControl`] of atomics. The backend retunes the control each
//! frame; the audio thread picks the new values up on the next sample frame, so a
//! moving source pans smoothly and the voice is never restarted.
//!
//! The stereo rule (Unity's practical behaviour):
//!   * a **mono** clip is copied to both channels, then panned;
//!   * a **stereo** clip is lerped toward its mono downmix `(l + r) / 2` by
//!     `spatial_blend`, then panned — fully 3D (`1`) is downmixed then panned, pure
//!     2D (`0`) keeps its stereo image untouched;
//!   * a clip with more than two channels uses its first two as left/right.
//!
//! Pan is a **balance** law: the far channel fades linearly to silence, the near one
//! stays at unity (`pan = -1` ⇒ right silent). A centred voice plays at unity on both
//! channels, exactly as it did before panning existed; loudness is the sink's gain.
//!
//! Platform layer only (the `RodioBackend` uses it); nothing in the sim reaches it.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;

/// The live pan state of one voice, shared between the backend (writer) and the
/// audio thread (reader). `f32`s stored as their bit patterns.
#[derive(Debug)]
pub struct PanControl {
    pan: AtomicU32,
    spatial_blend: AtomicU32,
}

impl PanControl {
    /// A control starting at `pan` / `spatial_blend`.
    pub fn new(pan: f32, spatial_blend: f32) -> Arc<Self> {
        let c = Arc::new(Self {
            pan: AtomicU32::new(0),
            spatial_blend: AtomicU32::new(0),
        });
        c.set(pan, spatial_blend);
        c
    }

    /// Retune the voice; clamps both into range.
    pub fn set(&self, pan: f32, spatial_blend: f32) {
        let pan = pan.clamp(-1.0, 1.0);
        let blend = spatial_blend.clamp(0.0, 1.0);
        self.pan.store(pan.to_bits(), Ordering::Relaxed);
        self.spatial_blend.store(blend.to_bits(), Ordering::Relaxed);
    }

    fn load(&self) -> (f32, f32) {
        (
            f32::from_bits(self.pan.load(Ordering::Relaxed)),
            f32::from_bits(self.spatial_blend.load(Ordering::Relaxed)),
        )
    }
}

/// Mix one input frame to a stereo `(left, right)` pair under the stereo rule and
/// balance pan law described in the module docs.
pub fn mix_frame(frame: &[f32], pan: f32, spatial_blend: f32) -> (f32, f32) {
    let (l, r) = match frame {
        [] => (0.0, 0.0),
        [mono] => (*mono, *mono),
        [l, r, ..] => {
            let mono = (l + r) * 0.5;
            (
                l + (mono - l) * spatial_blend,
                r + (mono - r) * spatial_blend,
            )
        }
    };
    let left_gain = (1.0 - pan).min(1.0);
    let right_gain = (1.0 + pan).min(1.0);
    (l * left_gain, r * right_gain)
}

/// A stereo source panning `inner` (`f32` samples — the backend converts the
/// decoded `i16`) per its [`PanControl`]. See the module docs.
pub struct PanSource<S> {
    inner: S,
    control: Arc<PanControl>,
    in_channels: usize,
    frame: Vec<f32>,
    /// The right-channel sample still owed for the current frame, if any.
    pending_right: Option<f32>,
}

impl<S> PanSource<S>
where
    S: Source<Item = f32>,
{
    pub fn new(inner: S, control: Arc<PanControl>) -> Self {
        let in_channels = usize::from(inner.channels().max(1));
        Self {
            inner,
            control,
            in_channels,
            frame: Vec::with_capacity(in_channels),
            pending_right: None,
        }
    }
}

impl<S> Iterator for PanSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.pending_right.take() {
            return Some(right);
        }
        self.frame.clear();
        for _ in 0..self.in_channels {
            self.frame.push(self.inner.next()?);
        }
        let (pan, blend) = self.control.load();
        let (left, right) = mix_frame(&self.frame, pan, blend);
        self.pending_right = Some(right);
        Some(left)
    }
}

impl<S> Source for PanSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        let owed = usize::from(self.pending_right.is_some());
        self.inner
            .current_frame_len()
            .map(|n| n / self.in_channels * 2 + owed)
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

#[cfg(test)]
#[path = "pan_tests.rs"]
mod pan_tests;
