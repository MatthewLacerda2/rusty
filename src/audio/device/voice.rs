//! src/audio/device/voice.rs — one playing voice, as a kira `Sound` (#212, #412, #465).
//!
//! Each voice is a [`ClipSound`] playing a cached clip into its mixer group's kira
//! track. It reads its live mix (gain, pan, `spatial_blend`, rate, pause, stop) from
//! a shared [`VoiceControl`] of atomics: the backend retunes the control each frame
//! and the audio thread picks the values up on its next buffer (a few ms), so a
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
//! channels. kira's own sounds pan with a constant-power law and keep no channel
//! count, which is why the voice is ours: kira does everything downstream of it
//! (groups, filters, sends, the speaker-mode stage, the device).
//!
//! The playhead advances `rate × clip rate` frames per second of output, with linear
//! interpolation between frames, so a clip at any sample rate plays at its pitch on
//! any device and `Time.timeScale` bends the pitch with the rate.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use kira::info::Info;
use kira::sound::{Sound, SoundData};
use kira::Frame;

use super::decode::CachedClip;
use crate::audio::backend::VoiceMix;

/// An `f32` shared between the backend (writer) and the audio thread (reader).
#[derive(Debug, Default)]
struct AtomicF32(AtomicU32);

impl AtomicF32 {
    fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// The live state of one voice, shared between the backend and the audio thread.
#[derive(Debug, Default)]
pub struct VoiceControl {
    gain: AtomicF32,
    pan: AtomicF32,
    spatial_blend: AtomicF32,
    speed: AtomicF32,
    paused: AtomicBool,
    /// Set by the backend to end the voice; kira then unloads it.
    stopped: AtomicBool,
    /// Set by the audio thread when a one-shot reaches its end.
    finished: AtomicBool,
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
        self.paused.store(mix.paused, Ordering::Relaxed);
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

/// One voice playing `clip`, looping or once, under its [`VoiceControl`].
pub struct ClipSound {
    clip: CachedClip,
    looping: bool,
    control: Arc<VoiceControl>,
    /// The playhead, in (fractional) clip frames.
    position: f64,
    /// Scratch for the interpolated frame (one sample per clip channel).
    frame: Vec<f32>,
}

impl ClipSound {
    pub fn new(clip: CachedClip, looping: bool, control: Arc<VoiceControl>) -> Self {
        let frame = vec![0.0; usize::from(clip.channels())];
        Self {
            clip,
            looping,
            control,
            position: 0.0,
            frame,
        }
    }

    /// The clip frame at `index`, wrapping for a looping voice; silence past the end.
    fn frame_at(&self, index: usize) -> &[f32] {
        let len = self.clip.frames();
        let index = if self.looping && len > 0 {
            index % len
        } else {
            index
        };
        self.clip.frame(index)
    }

    /// Interpolate the playhead's frame into `self.frame`.
    fn sample(&mut self) {
        let index = self.position as usize;
        let t = (self.position - index as f64) as f32;
        for c in 0..self.frame.len() {
            let a = self.frame_at(index).get(c).copied().unwrap_or(0.0);
            let b = self.frame_at(index + 1).get(c).copied().unwrap_or(0.0);
            self.frame[c] = a + (b - a) * t;
        }
    }

    fn run_out(&self) -> bool {
        !self.looping && self.position >= self.clip.frames() as f64
    }
}

impl Sound for ClipSound {
    fn process(&mut self, out: &mut [Frame], dt: f64, _info: &Info) {
        let c = &self.control;
        let (gain, pan, blend) = (c.gain.get(), c.pan.get(), c.spatial_blend.get());
        let step = f64::from(c.speed.get()) * f64::from(self.clip.sample_rate()) * dt;
        if c.paused.load(Ordering::Relaxed) || c.stopped.load(Ordering::Relaxed) {
            out.fill(Frame::ZERO);
            return;
        }
        for frame in out.iter_mut() {
            if self.run_out() {
                *frame = Frame::ZERO;
                continue;
            }
            self.sample();
            let (l, r) = mix_frame(&self.frame, pan, blend);
            *frame = Frame::new(l * gain, r * gain);
            self.position += step;
        }
        if self.looping && self.clip.frames() > 0 {
            self.position %= self.clip.frames() as f64;
        }
        if self.run_out() {
            self.control.finished.store(true, Ordering::Relaxed);
        }
    }

    fn finished(&self) -> bool {
        !self.control.is_live()
    }
}

/// The kira-side handle to start a [`ClipSound`]: the sound itself, no extra handle
/// (the backend keeps the [`VoiceControl`]).
pub struct ClipSoundData(pub ClipSound);

impl SoundData for ClipSoundData {
    type Error = ();
    type Handle = ();

    fn into_sound(self) -> Result<(Box<dyn Sound>, ()), ()> {
        Ok((Box::new(self.0), ()))
    }
}

#[cfg(test)]
#[path = "voice_tests.rs"]
mod voice_tests;
