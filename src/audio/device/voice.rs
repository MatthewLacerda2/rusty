//! src/audio/device/voice.rs — one playing voice, as a kira `Sound` (#212, #412, #465).
//!
//! Each voice is a [`ClipSound`] playing a cached clip into its mixer group's kira
//! track. It reads its live mix (gain, pan, `spatial_blend`, rate, doppler, pause,
//! stop) from a shared [`VoiceControl`] of atomics (`device/control.rs`): the backend retunes the control each frame
//! and the audio thread picks the values up on its next buffer (a few ms) and
//! glides to them (below), so a moving source pans smoothly and the voice is
//! never restarted.
//!
//! The stereo rule (Unity's practical behaviour):
//!   * a **mono** clip is copied to both channels, then panned;
//!   * a **stereo** clip is lerped toward its mono downmix `(l + r) / 2` by
//!     `spatial_blend`, then panned — fully 3D (`1`) is downmixed then panned, pure
//!     2D (`0`) keeps its stereo image untouched;
//!   * a clip with more than two channels uses its first two as left/right.
//!
//! Occlusion (#467) closes the voice's own low-pass ([`LowPass`]) after the pan.
//!
//! **Parameters glide, per sample** (#1693). The game retunes a voice once per
//! frame (every 25–40 ms at a low frame rate), so a gain, pan, blend, low-pass or
//! doppler rate (#1749) applied flat would jump at that rate: zipper noise, or a
//! chirp when a body's velocity steps. Each of the five instead
//! follows its target through a one-pole smoother with a [`SMOOTHING`] time
//! constant, as FMOD and Wwise ramp theirs. A voice's first sample plays at its
//! targets, with no fade-in, so a one-shot keeps its attack; a value that holds
//! still is played exactly, so an open filter stays bit-transparent and a voice
//! faded to zero gain is exact silence an effect can sleep over (#1729). The
//! time-scale rate, pause and stop apply at once, and a voice resumed from a pause starts on its
//! targets, like a new one.
//!
//! Pan is a **balance** law: the far channel fades linearly to silence, the near one
//! stays at unity (`pan = -1` ⇒ right silent). A centred voice plays at unity on both
//! channels. kira's own sounds pan with a constant-power law and keep no channel
//! count, which is why the voice is ours: kira does everything downstream of it
//! (groups, filters, sends, the speaker-mode stage, the device).
//!
//! The playhead advances `rate × doppler × clip rate` frames per second of output,
//! with linear interpolation between frames, so a clip at any sample rate plays at
//! its pitch on any device, and `Time.timeScale` and doppler bend the pitch.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use kira::info::Info;
use kira::sound::{Sound, SoundData};
use kira::Frame;

pub use super::control::VoiceControl;
use super::decode::CachedClip;
use super::lowpass::LowPass;

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

/// The time constant, in seconds, a voice's gain, pan, blend and low-pass follow
/// their targets with (#1693): ~99% of a change lands within 5 × this.
pub const SMOOTHING: f64 = 0.010;

/// Within this of its target, a gliding parameter lands on it exactly, so a
/// settled voice plays its values bit for bit (−120 dB on a unit gain).
/// A step too small to move an `f32` lands too: near `1.0` that stalls about
/// `3e-5` short (−90 dB), and would never arrive otherwise. So does a step out
/// of a NaN: a NaN target is played as long as the control holds it (about a game
/// frame, until the next retune), and the glide recovers the sample its target
/// does, never carrying the NaN for the voice's life.
const SNAP: f32 = 1e-6;

/// The parameters a voice glides (#1693, the doppler rate #1749). `low_pass` is the filter's one-pole
/// coefficient; `1.0` is open (the filter bypassed).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Glide {
    gain: f32,
    pan: f32,
    blend: f32,
    low_pass: f32,
    /// The doppler rate (#1749): glided so a velocity step never clicks.
    doppler: f32,
}

impl Glide {
    /// Move one sample toward `target`, by `k` of the remaining distance.
    fn toward(&mut self, target: &Glide, k: f32) {
        for (v, t) in [
            (&mut self.gain, target.gain),
            (&mut self.pan, target.pan),
            (&mut self.blend, target.blend),
            (&mut self.low_pass, target.low_pass),
            (&mut self.doppler, target.doppler),
        ] {
            let next = *v + k * (t - *v);
            *v = if next == *v || !next.is_finite() || (t - next).abs() <= SNAP {
                t
            } else {
                next
            };
        }
    }

    /// The low-pass coefficient [`LowPass::process`] takes (`None`: open).
    fn filter(&self) -> Option<f32> {
        (self.low_pass < 1.0).then_some(self.low_pass)
    }
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
    /// The voice's own low-pass, after the pan (#467).
    filter: LowPass,
    /// Where the gliding parameters stand; `None` until the first buffer, which
    /// starts them at their targets.
    glide: Option<Glide>,
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
            filter: LowPass::default(),
            glide: None,
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
        let target = Glide {
            gain: c.gain.get(),
            pan: c.pan.get(),
            blend: c.spatial_blend.get(),
            low_pass: LowPass::coefficient(c.low_pass.get(), dt).unwrap_or(1.0),
            doppler: c.doppler.get(),
        };
        let step = f64::from(c.speed.get()) * f64::from(self.clip.sample_rate()) * dt;
        if c.paused.load(Ordering::Relaxed) || c.stopped.load(Ordering::Relaxed) {
            // The output resumes from silence anyway, so there is nothing to glide
            // from: a voice retuned while paused resumes on its targets, as a new
            // one starts, never from the values it was paused at.
            self.glide = None;
            out.fill(Frame::ZERO);
            return;
        }
        let k = (1.0 - (-dt / SMOOTHING).exp()) as f32;
        let mut glide = *self.glide.get_or_insert(target);
        for frame in out.iter_mut() {
            if self.run_out() {
                *frame = Frame::ZERO;
                continue;
            }
            glide.toward(&target, k);
            self.sample();
            let mixed = mix_frame(&self.frame, glide.pan, glide.blend);
            let (l, r) = self.filter.process(glide.filter(), mixed);
            *frame = Frame::new(l * glide.gain, r * glide.gain);
            self.position += step * f64::from(glide.doppler);
        }
        self.glide = Some(glide);
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

#[cfg(test)]
#[path = "glide_tests.rs"]
mod glide_tests;

#[cfg(test)]
#[path = "doppler_glide_tests.rs"]
mod doppler_glide_tests;
